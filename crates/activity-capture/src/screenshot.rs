use hypr_activity_capture_interface::NormalizedSnapshot;
use hypr_activity_capture_interface::{ActivityKind, Snapshot, Transition, TransitionReason};
use hypr_screen_core::{WindowCaptureTarget, WindowContextCaptureOptions, WindowContextImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationScreenshotKind {
    Entry,
    Settled,
    Refresh,
}

impl ObservationScreenshotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Settled => "settled",
            Self::Refresh => "refresh",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "entry" => Self::Entry,
            "settled" => Self::Settled,
            "refresh" => Self::Refresh,
            _ => Self::Entry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationScreenshotTarget {
    pub window_id: Option<u32>,
    pub pid: u32,
    pub app_name: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationScreenshotRequest {
    pub request_id: u64,
    pub observation_id: String,
    pub observation_key: String,
    pub kind: ObservationScreenshotKind,
    pub scheduled_at_ms: i64,
    pub due_at_ms: i64,
    pub target: ObservationScreenshotTarget,
    pub snapshot: NormalizedSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationScreenshotCapture {
    pub request_id: u64,
    pub observation_id: String,
    pub observation_key: String,
    pub fingerprint: String,
    pub reason: String,
    pub kind: ObservationScreenshotKind,
    pub scheduled_at_ms: i64,
    pub captured_at_ms: i64,
    pub target: ObservationScreenshotTarget,
    pub snapshot: NormalizedSnapshot,
    pub image: WindowContextImage,
}

pub type ActivityScreenshotTarget = ObservationScreenshotTarget;
pub type ActivityScreenshotCapture = ObservationScreenshotCapture;

pub fn target_from_snapshot(snapshot: &NormalizedSnapshot) -> Option<ObservationScreenshotTarget> {
    Some(ObservationScreenshotTarget {
        window_id: snapshot.focused_window_id,
        pid: u32::try_from(snapshot.pid).ok()?,
        app_name: snapshot.app_name.clone(),
        title: snapshot
            .window_title
            .clone()
            .filter(|value| !value.is_empty()),
    })
}

pub fn capture_screenshot(
    target: &ObservationScreenshotTarget,
) -> Result<WindowContextImage, String> {
    hypr_screen_core::capture_target_window_context(
        &WindowCaptureTarget {
            window_id: target.window_id,
            pid: target.pid,
            app_name: Some(target.app_name.clone()),
            title: target.title.clone(),
        },
        WindowContextCaptureOptions::default(),
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone)]
pub struct ScreenshotConfig {
    pub dwell_ms: u64,
    pub min_interval_secs: u32,
    pub excluded_app_ids: Vec<String>,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            dwell_ms: 2_000,
            min_interval_secs: 6,
            excluded_app_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCapture {
    pub id: u64,
    pub fingerprint: String,
    pub reason: TransitionReason,
    pub cooldown_scope: String,
    pub scheduled_at_ms: i64,
    pub due_at_ms: i64,
    pub target: ObservationScreenshotTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotDecision {
    None,
    CancelPending,
    Schedule(PendingCapture),
    CancelAndSchedule(PendingCapture),
}

pub struct ScreenshotPolicy {
    config: ScreenshotConfig,
    next_id: u64,
    pending: Option<PendingCapture>,
    last_capture_ms_by_scope: std::collections::HashMap<String, i64>,
}

impl ScreenshotPolicy {
    pub fn new(config: ScreenshotConfig) -> Self {
        Self {
            config,
            next_id: 0,
            pending: None,
            last_capture_ms_by_scope: std::collections::HashMap::new(),
        }
    }

    pub fn on_transition(&mut self, transition: &Transition, now_ms: i64) -> ScreenshotDecision {
        if matches!(transition.reason, TransitionReason::Idle) || transition.current.is_none() {
            return self.clear_pending();
        }

        let current = match transition.current.as_ref() {
            Some(event) => event,
            None => return self.clear_pending(),
        };

        let snapshot = &current.snapshot;
        if !matches!(
            snapshot.activity_kind,
            ActivityKind::ForegroundWindow | ActivityKind::Browser
        ) {
            return self.clear_pending();
        }

        if !matches!(
            transition.reason,
            TransitionReason::Started
                | TransitionReason::AppChanged
                | TransitionReason::WindowChanged
        ) {
            return ScreenshotDecision::None;
        }

        if is_excluded_snapshot(snapshot, &self.config.excluded_app_ids) {
            return self.clear_pending();
        }

        let cooldown_scope = cooldown_scope(snapshot);
        if let Some(last) = self.last_capture_ms_by_scope.get(&cooldown_scope).copied() {
            let min_interval_ms = self.config.min_interval_secs as i64 * 1000;
            if now_ms - last < min_interval_ms {
                return self.clear_pending();
            }
        }

        let Some(target) = target_from_snapshot(snapshot) else {
            return self.clear_pending();
        };

        self.next_id += 1;
        let pending = PendingCapture {
            id: self.next_id,
            fingerprint: current.fingerprint.clone(),
            reason: transition.reason,
            cooldown_scope,
            scheduled_at_ms: now_ms,
            due_at_ms: now_ms.saturating_add(self.config.dwell_ms as i64),
            target,
        };

        self.replace_pending(pending)
    }

    pub fn fire(&mut self, pending_id: u64, now_ms: i64) -> Option<PendingCapture> {
        let pending = match self.pending.as_ref() {
            Some(pending) if pending.id == pending_id && now_ms >= pending.due_at_ms => {
                pending.clone()
            }
            _ => return None,
        };
        self.pending = None;
        self.last_capture_ms_by_scope
            .insert(pending.cooldown_scope.clone(), now_ms);
        Some(pending)
    }

    fn clear_pending(&mut self) -> ScreenshotDecision {
        if self.pending.take().is_some() {
            ScreenshotDecision::CancelPending
        } else {
            ScreenshotDecision::None
        }
    }

    fn replace_pending(&mut self, pending: PendingCapture) -> ScreenshotDecision {
        let had_pending = self.pending.replace(pending.clone()).is_some();
        if had_pending {
            ScreenshotDecision::CancelAndSchedule(pending)
        } else {
            ScreenshotDecision::Schedule(pending)
        }
    }
}

fn is_excluded_snapshot(snapshot: &Snapshot, excluded_app_ids: &[String]) -> bool {
    let candidates = [
        snapshot.app.bundle_id.as_deref(),
        Some(snapshot.app.app_id.as_str()),
        snapshot.app.executable_path.as_deref(),
    ];

    candidates
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|candidate| {
            excluded_app_ids
                .iter()
                .any(|excluded| excluded.trim() == candidate)
        })
}

fn cooldown_scope(snapshot: &Snapshot) -> String {
    snapshot
        .app
        .bundle_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(snapshot.app.app_id.as_str())
        .to_string()
}
