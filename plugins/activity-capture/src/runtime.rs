use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use futures_util::StreamExt;
use hypr_activity_capture::{
    ActivityCapture, BundleRule, CaptureAccess, CapturePolicy, ObservationReducer,
    ObservationReducerConfig, ObservationScreenshotCapture, ObservationScreenshotRequest,
    PlatformCapture, RawCaptureSample, WatchOptions, capture_screenshot,
};
use hypr_db_core2::Db3;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri_specta::Event;

use crate::{
    analysis::{self, ANALYSIS_MODEL_NAME, ANALYSIS_PROMPT_VERSION},
    events::{
        ActivityCaptureConfig, ActivityCaptureObservation, ActivityCaptureObservationAnalysis,
        ActivityCaptureObservationAnalysisError, ActivityCaptureObservationEvent,
        ActivityCapturePluginEvent, ActivityCaptureRuntimeError, ActivityCaptureStateChanged,
        ActivityCaptureStatus, ObservationEventKind, system_time_to_unix_ms, unix_ms_now,
    },
};

#[derive(Clone, Default)]
struct LastKnown {
    state_changed_at_ms: Option<i64>,
    current_observation: Option<ActivityCaptureObservation>,
    observation_event: Option<ActivityCaptureObservationEvent>,
    error: Option<ActivityCaptureRuntimeError>,
    observation_analysis: Option<ActivityCaptureObservationAnalysis>,
    observation_analysis_error: Option<ActivityCaptureObservationAnalysisError>,
}

const KNOWN_DESKTOP_APP_IDS: &[&str] = &[
    "com.hyprnote.dev",
    "com.hyprnote.stable",
    "com.hyprnote.staging",
    "com.hyprnote.nightly",
    "com.hyprnote.Hyprnote",
];
const WATCH_POLL_INTERVAL_MS: u64 = 750;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SelfIdentity {
    excluded_app_ids: Vec<String>,
}

impl SelfIdentity {
    fn resolve<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Self {
        Self::from_parts(
            Some(app.config().identifier.clone()),
            std::env::current_exe().ok(),
        )
    }

    fn from_parts(app_identifier: Option<String>, executable_path: Option<PathBuf>) -> Self {
        let mut excluded_app_ids = Vec::new();

        if let Some(app_identifier) = normalize_identity(app_identifier) {
            excluded_app_ids.push(app_identifier.clone());
            if KNOWN_DESKTOP_APP_IDS.contains(&app_identifier.as_str()) {
                excluded_app_ids
                    .extend(KNOWN_DESKTOP_APP_IDS.iter().map(|value| value.to_string()));
            }
        }

        if let Some(executable_path) = executable_path
            .map(|value| value.to_string_lossy().trim().to_string())
            .filter(|value| !value.is_empty())
        {
            excluded_app_ids.push(executable_path);
        }

        Self {
            excluded_app_ids: dedupe_identities(excluded_app_ids),
        }
    }
}

fn normalize_identity(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn dedupe_identities(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

pub struct ActivityCaptureRuntime<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    db: Arc<Db3>,
    policy: Mutex<CapturePolicy>,
    config: ActivityCaptureConfig,
    analyze_screenshots: AtomicBool,
    running: AtomicBool,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    reducer: Mutex<ObservationReducer>,
    last_known: Mutex<LastKnown>,
}

impl<R: tauri::Runtime> ActivityCaptureRuntime<R> {
    pub fn new(app: tauri::AppHandle<R>, db: Arc<Db3>) -> Self {
        let excluded_app_ids = SelfIdentity::resolve(&app).excluded_app_ids;
        let config = ActivityCaptureConfig {
            poll_interval_ms: WATCH_POLL_INTERVAL_MS,
            entry_dwell_ms: 1_200,
            typing_settle_ms: 2_500,
            long_typing_checkpoint_ms: 10_000,
            refresh_interval_ms: 60_000,
        };

        Self {
            app,
            db,
            policy: Mutex::new(CapturePolicy {
                app_rules: excluded_app_ids
                    .iter()
                    .map(|bundle_id| BundleRule {
                        bundle_id: bundle_id.clone(),
                        access: CaptureAccess::None,
                    })
                    .collect(),
                ..CapturePolicy::default()
            }),
            config,
            analyze_screenshots: AtomicBool::new(true),
            running: AtomicBool::new(false),
            task: Mutex::new(None),
            reducer: Mutex::new(ObservationReducer::new(ObservationReducerConfig {
                entry_dwell_ms: config.entry_dwell_ms,
                typing_settle_ms: config.typing_settle_ms,
                long_typing_checkpoint_ms: config.long_typing_checkpoint_ms,
                refresh_interval_ms: config.refresh_interval_ms,
            })),
            last_known: Mutex::new(LastKnown::default()),
        }
    }

    pub fn pool(&self) -> &hypr_db_core2::DbPool {
        self.db.pool()
    }

    pub fn configure(&self, analyze_screenshots: Option<bool>) -> Result<(), crate::Error> {
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

    pub fn current_observation(&self) -> Option<ActivityCaptureObservation> {
        self.last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current_observation
            .clone()
    }

    pub fn latest_observation_analysis(&self) -> Option<ActivityCaptureObservationAnalysis> {
        self.last_known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .observation_analysis
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
            hypr_db_app::count_screenshots_since(self.db.pool(), one_hour_ago)
                .await
                .unwrap_or(0);
        let screenshots_today = hypr_db_app::count_screenshots_since(self.db.pool(), today_start)
            .await
            .unwrap_or(0);
        let storage_bytes = hypr_db_app::total_screenshot_storage_bytes(self.db.pool())
            .await
            .unwrap_or(0);

        ActivityCaptureStatus {
            is_running: self.is_running(),
            last_state_changed_at_ms: last.state_changed_at_ms,
            current_observation: last.current_observation,
            last_observation_event: last.observation_event,
            last_error: last.error,
            last_observation_analysis: last.observation_analysis,
            last_observation_analysis_error: last.observation_analysis_error,
            config: self.config,
            analyze_screenshots: self.analyze_screenshots.load(Ordering::SeqCst),
            screenshots_today,
            screenshots_this_hour,
            storage_used_mb: storage_bytes / (1024 * 1024),
        }
    }

    pub async fn list_observation_analyses_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<ActivityCaptureObservationAnalysis>, String> {
        let rows = hypr_db_app::list_preferred_observation_analyses_in_range(
            self.db.pool(),
            start_ms,
            end_ms,
        )
        .await
        .map_err(|error| error.to_string())?;

        Ok(rows
            .into_iter()
            .map(|row| ActivityCaptureObservationAnalysis {
                observation_id: row.observation_id,
                screenshot_id: row.screenshot_id,
                screenshot_kind: row.screenshot_kind,
                captured_at_ms: row.captured_at_ms,
                app_name: row.app_name,
                window_title: (!row.window_title.is_empty()).then_some(row.window_title),
                summary: row.summary,
            })
            .collect())
    }

    pub fn start(self: &Arc<Self>) -> Result<(), crate::Error> {
        if self.is_running() {
            return Ok(());
        }

        let capture = PlatformCapture::with_policy(self.policy());
        let mut stream = capture.watch(WatchOptions {
            poll_interval: Duration::from_millis(self.config.poll_interval_ms),
            emit_initial: true,
        })?;

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
        let _ = event.emit(&self.app);

        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(sample) => runtime.handle_capture_sample(sample).await,
                    Err(error) => {
                        let runtime_error = ActivityCaptureRuntimeError {
                            kind: error.kind,
                            message: error.message,
                            occurred_at_ms: unix_ms_now(),
                        };
                        {
                            let mut last =
                                runtime.last_known.lock().unwrap_or_else(|e| e.into_inner());
                            last.error = Some(runtime_error.clone());
                            last.state_changed_at_ms = Some(runtime_error.occurred_at_ms);
                        }
                        let _ = ActivityCapturePluginEvent::Error {
                            error: runtime_error,
                        }
                        .emit(&runtime.app);
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
            let _ = ActivityCapturePluginEvent::StateChanged {
                state: ActivityCaptureStateChanged {
                    is_running: false,
                    changed_at_ms,
                },
            }
            .emit(&runtime.app);
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
        let _ = ActivityCapturePluginEvent::StateChanged {
            state: ActivityCaptureStateChanged {
                is_running: false,
                changed_at_ms,
            },
        }
        .emit(&self.app);
    }

    async fn handle_capture_sample(self: &Arc<Self>, sample: RawCaptureSample) {
        let update = {
            let mut reducer = self.reducer.lock().unwrap_or_else(|e| e.into_inner());
            let update = reducer.ingest(sample.captured_at, sample.snapshot);
            let current = reducer.current_observation().map(Into::into);
            self.last_known
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .current_observation = current;
            update
        };

        for event in update.events {
            self.persist_observation_event(&event).await;
            let event_payload: ActivityCaptureObservationEvent = event.clone().into();
            self.last_known
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .observation_event = Some(event_payload.clone());
            let plugin_event = match event.kind {
                ObservationEventKind::Started => ActivityCapturePluginEvent::ObservationStarted {
                    event: event_payload,
                },
                ObservationEventKind::Checkpointed => {
                    ActivityCapturePluginEvent::ObservationCheckpointed {
                        event: event_payload,
                    }
                }
                ObservationEventKind::Ended => ActivityCapturePluginEvent::ObservationEnded {
                    event: event_payload,
                },
            };
            let _ = plugin_event.emit(&self.app);
        }

        for request in update.capture_requests {
            self.handle_screenshot_request(request).await;
        }
    }

    async fn handle_screenshot_request(self: &Arc<Self>, request: ObservationScreenshotRequest) {
        let image = match capture_screenshot(&request.target) {
            Ok(image) => image,
            Err(error) => {
                let _ = self
                    .reducer
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .acknowledge_capture(request.request_id, false, unix_ms_now());
                tracing::warn!(%error, request_id = request.request_id, "activity_screenshot_capture_failed");
                return;
            }
        };

        let captured_at_ms = image.captured_at_ms;
        let capture = ObservationScreenshotCapture {
            request_id: request.request_id,
            observation_id: request.observation_id.clone(),
            observation_key: request.observation_key.clone(),
            fingerprint: request.snapshot.content_fingerprint(),
            reason: request.kind.as_str().to_string(),
            kind: request.kind,
            scheduled_at_ms: request.scheduled_at_ms,
            captured_at_ms,
            target: request.target,
            snapshot: request.snapshot,
            image,
        };

        let _ = self
            .reducer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .acknowledge_capture(request.request_id, true, captured_at_ms);

        let screenshot_id = screenshot_id_for(&capture);
        if let Err(error) = persist_screenshot(self.db.pool(), &screenshot_id, &capture).await {
            tracing::warn!(%error, "failed_to_persist_activity_screenshot");
        }

        if self.analyze_screenshots.load(Ordering::SeqCst) {
            self.spawn_screenshot_analysis(screenshot_id, capture);
        }
    }

    fn spawn_screenshot_analysis(
        self: &Arc<Self>,
        screenshot_id: String,
        capture: ObservationScreenshotCapture,
    ) {
        let runtime = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            match analysis::analyze_screenshot(&runtime.app, &screenshot_id, &capture).await {
                Ok(analysis) => {
                    if let Err(error) = hypr_db_app::insert_observation_analysis(
                        runtime.db.pool(),
                        hypr_db_app::InsertObservationAnalysis {
                            id: &format!(
                                "oa-{}-{}-{}",
                                analysis.observation_id,
                                analysis.screenshot_id,
                                ANALYSIS_PROMPT_VERSION
                            ),
                            observation_id: &analysis.observation_id,
                            screenshot_id: &analysis.screenshot_id,
                            screenshot_kind: &analysis.screenshot_kind,
                            captured_at_ms: analysis.captured_at_ms,
                            model_name: ANALYSIS_MODEL_NAME,
                            prompt_version: ANALYSIS_PROMPT_VERSION,
                            app_name: &analysis.app_name,
                            window_title: analysis.window_title.as_deref().unwrap_or(""),
                            summary: &analysis.summary,
                        },
                    )
                    .await
                    {
                        tracing::warn!(%error, "failed_to_persist_observation_analysis");
                    }

                    {
                        let mut last = runtime.last_known.lock().unwrap_or_else(|e| e.into_inner());
                        last.observation_analysis = Some(analysis.clone());
                        last.observation_analysis_error = None;
                    }
                    let _ = ActivityCapturePluginEvent::ObservationAnalysisReady { analysis }
                        .emit(&runtime.app);
                }
                Err(error) => {
                    runtime
                        .last_known
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .observation_analysis_error = Some(error.clone());
                    let _ = ActivityCapturePluginEvent::ObservationAnalysisError { error }
                        .emit(&runtime.app);
                }
            }
        });
    }

    async fn persist_observation_event(&self, event: &hypr_activity_capture::ObservationEvent) {
        let snapshot = event.snapshot.as_ref();
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
        let snapshot_json = snapshot
            .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "{}".to_string());

        if let Err(error) = hypr_db_app::insert_observation_event(
            self.db.pool(),
            hypr_db_app::InsertObservationEvent {
                id: &event.id,
                observation_id: &event.observation_id,
                occurred_at_ms: system_time_to_unix_ms(event.occurred_at),
                event_kind: event.kind.as_str(),
                end_reason: event.end_reason.map(|value| value.as_str()),
                change_class: event.change_class.map(|value| value.as_str()),
                app_id,
                bundle_id,
                app_name,
                activity_kind: snapshot.map(|s| s.activity_kind.as_str()).unwrap_or(""),
                window_title,
                url,
                domain: &domain,
                text_anchor_identity: snapshot
                    .and_then(|s| s.text_anchor_identity.as_deref())
                    .unwrap_or(""),
                observation_key: &event.observation_key,
                snapshot_json: &snapshot_json,
            },
        )
        .await
        {
            tracing::warn!(%error, "failed_to_persist_observation_event");
        }
    }
}

async fn persist_screenshot(
    pool: &SqlitePool,
    screenshot_id: &str,
    capture: &ObservationScreenshotCapture,
) -> Result<(), sqlx::Error> {
    let sha256 = Sha256::digest(&capture.image.image_bytes);
    let sha256 = sha256
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let snapshot_json =
        serde_json::to_string(&capture.snapshot).unwrap_or_else(|_| "{}".to_string());

    hypr_db_app::insert_screenshot(
        pool,
        hypr_db_app::InsertScreenshot {
            id: screenshot_id,
            observation_id: &capture.observation_id,
            screenshot_kind: capture.kind.as_str(),
            scheduled_at_ms: capture.scheduled_at_ms,
            captured_at_ms: capture.captured_at_ms,
            app_name: &capture.target.app_name,
            window_title: capture.target.title.as_deref().unwrap_or(""),
            mime_type: &capture.image.mime_type,
            width: capture.image.width as i64,
            height: capture.image.height as i64,
            sha256: &sha256,
            image_blob: &capture.image.image_bytes,
            snapshot_json: &snapshot_json,
        },
    )
    .await
}

fn screenshot_id_for(capture: &ObservationScreenshotCapture) -> String {
    format!(
        "ss-{}-{}-{}-{}",
        capture.observation_id,
        capture.kind.as_str(),
        capture.captured_at_ms,
        capture.request_id
    )
}

#[cfg(test)]
mod tests {
    use super::{ActivityCaptureRuntime, SelfIdentity};
    use hypr_db_core2::Db3;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn self_identity_includes_app_identifier() {
        let identity = SelfIdentity::from_parts(Some("com.hyprnote.stable".to_string()), None);

        assert!(
            identity
                .excluded_app_ids
                .contains(&"com.hyprnote.stable".to_string())
        );
    }

    #[test]
    fn self_identity_includes_executable_path() {
        let identity = SelfIdentity::from_parts(
            Some("com.hyprnote.stable".to_string()),
            Some(PathBuf::from("/Applications/Char.app/Contents/MacOS/Char")),
        );

        assert!(
            identity
                .excluded_app_ids
                .contains(&"/Applications/Char.app/Contents/MacOS/Char".to_string())
        );
    }

    #[test]
    fn runtime_initializes_with_self_exclusions() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let runtime = ActivityCaptureRuntime::new(
            app.handle().clone(),
            Arc::new(rt.block_on(async { Db3::connect_memory_plain().await.unwrap() })),
        );

        assert!(!runtime.policy().app_rules.is_empty());
    }
}
