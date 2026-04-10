use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures_util::StreamExt;
use hypr_activity_capture::{
    ActivityCapture, ActivityScreenshotCapture, CapturePolicy, PlatformCapture, ScreenshotConfig,
    ScreenshotDecision, ScreenshotPolicy, capture_screenshot,
};
use sqlx::SqlitePool;
use tauri_specta::Event;

use crate::{
    analysis,
    events::{
        ActivityCaptureBudget, ActivityCapturePluginEvent, ActivityCaptureRuntimeError,
        ActivityCaptureScreenshotAnalysis, ActivityCaptureScreenshotAnalysisError,
        ActivityCaptureSignal, ActivityCaptureStateChanged, ActivityCaptureStatus,
        CaptureErrorKind, unix_ms_now,
    },
};

#[derive(Clone, Default)]
struct LastKnown {
    state_changed_at_ms: Option<i64>,
    signal: Option<ActivityCaptureSignal>,
    error: Option<ActivityCaptureRuntimeError>,
    screenshot_analysis: Option<ActivityCaptureScreenshotAnalysis>,
    screenshot_analysis_error: Option<ActivityCaptureScreenshotAnalysisError>,
}

pub struct ActivityCaptureRuntime<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    pool: Arc<SqlitePool>,
    policy: Mutex<CapturePolicy>,
    screenshot_config: ScreenshotConfig,
    analyze_screenshots: AtomicBool,
    running: AtomicBool,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    screenshot_policy: Mutex<ScreenshotPolicy>,
    latest_capture: Mutex<Option<ActivityScreenshotCapture>>,
    last_known: Mutex<LastKnown>,
    screenshot_task: Mutex<ScreenshotTaskState>,
    recent_analyses: Mutex<HashMap<String, i64>>,
}

impl<R: tauri::Runtime> ActivityCaptureRuntime<R> {
    pub fn new(app: tauri::AppHandle<R>, pool: Arc<SqlitePool>) -> Self {
        let screenshot_config = ScreenshotConfig::default();
        Self {
            app,
            pool,
            policy: Mutex::new(CapturePolicy::default()),
            screenshot_config,
            analyze_screenshots: AtomicBool::new(true),
            running: AtomicBool::new(false),
            task: Mutex::new(None),
            screenshot_policy: Mutex::new(ScreenshotPolicy::new(screenshot_config)),
            latest_capture: Mutex::new(None),
            last_known: Mutex::new(LastKnown::default()),
            screenshot_task: Mutex::new(ScreenshotTaskState::default()),
            recent_analyses: Mutex::new(HashMap::new()),
        }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn configure(
        &self,
        _budget: Option<ActivityCaptureBudget>,
        analyze_screenshots: Option<bool>,
    ) -> Result<(), crate::Error> {
        if let Some(analyze) = analyze_screenshots {
            self.analyze_screenshots.store(analyze, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn policy(&self) -> CapturePolicy {
        self.policy
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_policy(self: &Arc<Self>, policy: CapturePolicy) -> Result<(), crate::Error> {
        *self.policy.lock().unwrap_or_else(|e| e.into_inner()) = policy;
        if self.is_running() {
            self.restart()?;
        }
        Ok(())
    }

    pub fn reset_policy(self: &Arc<Self>) -> Result<(), crate::Error> {
        self.set_policy(CapturePolicy::default())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn latest_screenshot_analysis(&self) -> Option<ActivityCaptureScreenshotAnalysis> {
        self.last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .screenshot_analysis
            .clone()
    }

    pub async fn status(&self) -> ActivityCaptureStatus {
        let last = self
            .last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let now = unix_ms_now();
        let one_hour_ago = now - 3_600_000;
        let today_start = now - (now % 86_400_000);

        let screenshots_this_hour =
            hypr_db_activity::count_screenshots_since(&self.pool, one_hour_ago)
                .await
                .unwrap_or(0);

        let screenshots_today = hypr_db_activity::count_screenshots_since(&self.pool, today_start)
            .await
            .unwrap_or(0);

        let storage_bytes = hypr_db_activity::total_screenshot_storage_bytes(&self.pool)
            .await
            .unwrap_or(0);

        ActivityCaptureStatus {
            is_running: self.is_running(),
            last_state_changed_at_ms: last.state_changed_at_ms,
            last_signal: last.signal.clone(),
            last_error: last.error.clone(),
            last_screenshot_analysis: last.screenshot_analysis.clone(),
            last_screenshot_analysis_error: last.screenshot_analysis_error.clone(),
            budget: ActivityCaptureBudget {
                min_interval_secs: self.screenshot_config.min_interval_secs,
            },
            analyze_screenshots: self.analyze_screenshots.load(Ordering::SeqCst),
            screenshots_today,
            screenshots_this_hour,
            storage_used_mb: storage_bytes / (1024 * 1024),
        }
    }

    pub async fn list_analyses_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<ActivityCaptureScreenshotAnalysis>, String> {
        let rows =
            hypr_db_activity::list_screenshot_analyses_in_range(&self.pool, start_ms, end_ms)
                .await
                .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|row| ActivityCaptureScreenshotAnalysis {
                fingerprint: row.fingerprint,
                reason: hypr_activity_capture::TransitionReason::parse(&row.reason),
                captured_at_ms: row.captured_at_ms,
                app_name: row.app_name,
                window_title: if row.window_title.is_empty() {
                    None
                } else {
                    Some(row.window_title)
                },
                summary: row.analysis_summary,
            })
            .collect())
    }

    async fn persist_signal(&self, signal: &ActivityCaptureSignal) {
        let id = format!("sig-{}-{}", signal.occurred_at_ms, signal.sequence);
        let snapshot = signal.snapshot.as_ref();
        let reason = signal.reason.as_str();
        let fingerprint = signal.fingerprint.as_deref().unwrap_or("");
        let app_id = snapshot.map(|s| s.app.app_id.as_str()).unwrap_or("");
        let bundle_id = snapshot
            .and_then(|s| s.app.bundle_id.as_deref())
            .unwrap_or("");
        let app_name = snapshot.map(|s| s.app_name.as_str()).unwrap_or("");
        let window_title = snapshot
            .and_then(|s| s.window_title.as_deref())
            .unwrap_or("");
        let url = snapshot.and_then(|s| s.url.as_deref()).unwrap_or("");

        let domain = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_default();

        if let Err(error) = hypr_db_activity::insert_signal(
            &self.pool,
            hypr_db_activity::InsertSignal {
                id: &id,
                occurred_at_ms: signal.occurred_at_ms,
                transition_sequence: signal.sequence,
                reason,
                app_id,
                bundle_id,
                app_name,
                activity_kind: snapshot.map(|s| s.activity_kind.as_str()).unwrap_or(""),
                window_title,
                url,
                domain: &domain,
                content_level: snapshot.map(|s| s.content_level.as_str()).unwrap_or(""),
                source: snapshot.map(|s| s.source.as_str()).unwrap_or(""),
                text_anchor_identity: snapshot
                    .and_then(|s| s.text_anchor_identity.as_deref())
                    .unwrap_or(""),
                fingerprint,
                payload_json: "{}",
            },
        )
        .await
        {
            tracing::warn!(%error, "failed_to_persist_activity_signal");
        }
    }

    pub fn start(self: &Arc<Self>) -> Result<(), crate::Error> {
        if self.is_running() {
            return Ok(());
        }

        let capture = PlatformCapture::with_policy(self.policy());
        let mut stream = capture.watch(Default::default())?;

        let changed_at_ms = unix_ms_now();
        self.last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state_changed_at_ms = Some(changed_at_ms);
        self.running.store(true, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }
        let event = ActivityCapturePluginEvent::StateChanged {
            state: ActivityCaptureStateChanged {
                is_running: true,
                changed_at_ms,
            },
        };
        if let Err(error) = event.emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_activity_capture_state_changed");
        }

        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(transition) => {
                        runtime.handle_screenshot_transition(&transition);
                        let signal = ActivityCaptureSignal::from(transition);
                        runtime.persist_signal(&signal).await;
                        runtime
                            .last_known
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .signal = Some(signal.clone());
                        let event = ActivityCapturePluginEvent::Signal { signal };
                        if let Err(error) = event.emit(&runtime.app) {
                            tracing::error!(?error, "failed_to_emit_activity_capture_signal");
                        }
                    }
                    Err(error) => {
                        let runtime_error = ActivityCaptureRuntimeError {
                            kind: CaptureErrorKind::from(error.kind),
                            message: error.message,
                            occurred_at_ms: unix_ms_now(),
                        };
                        {
                            let mut last =
                                runtime.last_known.lock().unwrap_or_else(|e| e.into_inner());
                            last.error = Some(runtime_error.clone());
                            last.state_changed_at_ms = Some(runtime_error.occurred_at_ms);
                        }
                        let event = ActivityCapturePluginEvent::Error {
                            error: runtime_error,
                        };
                        if let Err(emit_error) = event.emit(&runtime.app) {
                            tracing::error!(?emit_error, "failed_to_emit_activity_capture_error");
                        }
                        break;
                    }
                }
            }

            runtime.running.store(false, Ordering::SeqCst);
            let changed_at_ms = unix_ms_now();
            runtime
                .last_known
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .state_changed_at_ms = Some(changed_at_ms);
            let event = ActivityCapturePluginEvent::StateChanged {
                state: ActivityCaptureStateChanged {
                    is_running: false,
                    changed_at_ms,
                },
            };
            if let Err(error) = event.emit(&runtime.app) {
                tracing::error!(?error, "failed_to_emit_activity_capture_state_changed");
            }
        });

        *self.task.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    fn restart(self: &Arc<Self>) -> Result<(), crate::Error> {
        self.stop();
        self.start()
    }

    pub fn stop(&self) {
        let changed_at_ms = unix_ms_now();
        self.last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state_changed_at_ms = Some(changed_at_ms);
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }
        if let Some(handle) = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .handle
            .take()
        {
            handle.abort();
        }
        let event = ActivityCapturePluginEvent::StateChanged {
            state: ActivityCaptureStateChanged {
                is_running: false,
                changed_at_ms,
            },
        };
        if let Err(error) = event.emit(&self.app) {
            tracing::error!(?error, "failed_to_emit_activity_capture_state_changed");
        }
    }

    fn handle_screenshot_transition(
        self: &Arc<Self>,
        transition: &hypr_activity_capture::Transition,
    ) {
        let decision = {
            let mut policy = self
                .screenshot_policy
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            policy.on_transition(transition, unix_ms_now())
        };

        match decision {
            ScreenshotDecision::None => {}
            ScreenshotDecision::CancelPending => self.clear_screenshot_task(),
            ScreenshotDecision::Schedule(pending) => self.replace_screenshot_task(pending),
            ScreenshotDecision::CancelAndSchedule(pending) => {
                self.clear_screenshot_task();
                self.replace_screenshot_task(pending);
            }
        }
    }

    fn clear_screenshot_task(&self) {
        let mut task_state = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        task_state.pending_id = None;
        if let Some(handle) = task_state.handle.take() {
            handle.abort();
        }
    }

    fn replace_screenshot_task(self: &Arc<Self>, pending: hypr_activity_capture::PendingCapture) {
        let delay_ms = pending.due_at_ms.saturating_sub(unix_ms_now()).max(0) as u64;
        let pending_id = pending.id;
        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            runtime.fire_screenshot_capture(pending_id);
        });

        let mut task_state = self
            .screenshot_task
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = task_state.handle.take() {
            existing.abort();
        }
        task_state.pending_id = Some(pending_id);
        task_state.handle = Some(handle);
    }

    fn fire_screenshot_capture(self: &Arc<Self>, pending_id: u64) {
        {
            let mut task_state = self
                .screenshot_task
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if task_state.pending_id == Some(pending_id) {
                task_state.pending_id = None;
                task_state.handle = None;
            }
        }

        let now_ms = unix_ms_now();
        let pending = {
            let mut policy = self
                .screenshot_policy
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match policy.fire(pending_id, now_ms) {
                Some(p) => p,
                None => return,
            }
        };

        let image = match capture_screenshot(&pending.target) {
            Ok(image) => image,
            Err(error) => {
                tracing::warn!(pending_id, %error, "activity_screenshot_capture_failed");
                return;
            }
        };

        let captured_at_ms = image.captured_at_ms;
        let capture = ActivityScreenshotCapture {
            fingerprint: pending.fingerprint,
            reason: pending.reason,
            scheduled_at_ms: pending.scheduled_at_ms,
            captured_at_ms,
            target: pending.target,
            image,
        };

        tracing::info!(
            fingerprint = %capture.fingerprint,
            reason = ?capture.reason,
            pid = capture.target.pid,
            app_name = %capture.target.app_name,
            title = capture.target.title.as_deref().unwrap_or_default(),
            scheduled_at_ms = capture.scheduled_at_ms,
            captured_at_ms = capture.captured_at_ms,
            "activity_screenshot_capture_succeeded"
        );

        *self
            .latest_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(capture.clone());

        if self.should_analyze(&capture) {
            self.spawn_screenshot_analysis(capture);
        } else {
            self.spawn_screenshot_persist_only(capture);
        }
    }

    fn should_analyze(&self, capture: &ActivityScreenshotCapture) -> bool {
        use hypr_activity_capture::TransitionReason;

        if !self.analyze_screenshots.load(Ordering::SeqCst) {
            return false;
        }

        if matches!(
            capture.reason,
            TransitionReason::TitleChanged | TransitionReason::UrlChanged
        ) {
            return false;
        }

        let now = unix_ms_now();
        let dedup_window_ms: i64 = 5 * 60 * 1000;
        let mut cache = self
            .recent_analyses
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.retain(|_, ts| now - *ts < dedup_window_ms);
        if let Some(&last_ts) = cache.get(&capture.fingerprint) {
            if now - last_ts < dedup_window_ms {
                return false;
            }
        }
        cache.insert(capture.fingerprint.clone(), now);

        true
    }

    fn spawn_screenshot_persist_only(self: &Arc<Self>, capture: ActivityScreenshotCapture) {
        let pool = Arc::clone(&self.pool);
        tauri::async_runtime::spawn(async move {
            if let Err(error) = persist_screenshot(&pool, &capture).await {
                tracing::warn!(%error, "failed_to_persist_activity_screenshot");
            }
        });
    }

    fn spawn_screenshot_analysis(self: &Arc<Self>, capture: ActivityScreenshotCapture) {
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            if let Err(error) = persist_screenshot(&runtime.pool, &capture).await {
                tracing::warn!(%error, "failed_to_persist_activity_screenshot");
            }

            let screenshot_id =
                format!("ss-{}-{}", capture.captured_at_ms, capture.scheduled_at_ms);

            match analysis::analyze_screenshot(&runtime.app, &capture).await {
                Ok(analysis) => {
                    if let Err(error) = hypr_db_activity::update_screenshot_analysis(
                        &runtime.pool,
                        &screenshot_id,
                        &analysis.summary,
                        unix_ms_now(),
                    )
                    .await
                    {
                        tracing::warn!(%error, "failed_to_persist_screenshot_analysis");
                    }

                    {
                        let mut last = runtime.last_known.lock().unwrap_or_else(|e| e.into_inner());
                        last.screenshot_analysis = Some(analysis.clone());
                        last.screenshot_analysis_error = None;
                    }
                    let event = ActivityCapturePluginEvent::ScreenshotAnalysis { analysis };
                    if let Err(error) = event.emit(&runtime.app) {
                        tracing::error!(
                            ?error,
                            "failed_to_emit_activity_capture_screenshot_analysis"
                        );
                    }
                }
                Err(error) => {
                    runtime
                        .last_known
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .screenshot_analysis_error = Some(error.clone());
                    let event = ActivityCapturePluginEvent::ScreenshotAnalysisError { error };
                    if let Err(emit_error) = event.emit(&runtime.app) {
                        tracing::error!(
                            ?emit_error,
                            "failed_to_emit_activity_capture_screenshot_analysis_error"
                        );
                    }
                }
            }
        });
    }
}

#[derive(Default)]
struct ScreenshotTaskState {
    pending_id: Option<u64>,
    handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

async fn persist_screenshot(
    pool: &SqlitePool,
    capture: &ActivityScreenshotCapture,
) -> Result<(), sqlx::Error> {
    let screenshot_id = format!("ss-{}-{}", capture.captured_at_ms, capture.scheduled_at_ms);
    let signal_id = format!("sig-{}-{}", capture.scheduled_at_ms, capture.captured_at_ms);

    hypr_db_activity::insert_screenshot(
        pool,
        hypr_db_activity::InsertScreenshot {
            id: &screenshot_id,
            signal_id: &signal_id,
            fingerprint: &capture.fingerprint,
            captured_at_ms: capture.captured_at_ms,
            image_png: &capture.image.image_bytes,
        },
    )
    .await
}
