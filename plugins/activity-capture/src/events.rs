use std::time::{SystemTime, UNIX_EPOCH};

use hypr_activity_capture as core;

pub use core::{
    ActivityKind, AppIdKind, CaptureErrorKind, ContentLevel, SnapshotSource, TextAnchorConfidence,
    TextAnchorKind, TransitionReason,
};

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureSnapshot {
    pub app: ActivityCaptureAppIdentity,
    pub activity_kind: ActivityKind,
    pub captured_at_ms: i64,
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub focused_window_id: Option<u32>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub text_anchor_kind: Option<TextAnchorKind>,
    pub text_anchor_identity: Option<String>,
    pub text_anchor_text: Option<String>,
    pub text_anchor_prefix: Option<String>,
    pub text_anchor_suffix: Option<String>,
    pub text_anchor_selected_text: Option<String>,
    pub text_anchor_confidence: Option<TextAnchorConfidence>,
    pub content_level: ContentLevel,
    pub source: SnapshotSource,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureAppIdentity {
    pub pid: i32,
    pub app_name: String,
    pub app_id: String,
    pub app_id_kind: AppIdKind,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureSignal {
    pub sequence: i64,
    pub occurred_at_ms: i64,
    pub reason: TransitionReason,
    pub suppressed_snapshot_count: i32,
    pub fingerprint: Option<String>,
    pub snapshot: Option<ActivityCaptureSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureScreenshotAnalysis {
    pub fingerprint: String,
    pub reason: TransitionReason,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureScreenshotAnalysisError {
    pub fingerprint: String,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureRuntimeError {
    pub kind: CaptureErrorKind,
    pub message: String,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureStatus {
    pub is_running: bool,
    pub last_state_changed_at_ms: Option<i64>,
    pub last_signal: Option<ActivityCaptureSignal>,
    pub last_error: Option<ActivityCaptureRuntimeError>,
    pub last_screenshot_analysis: Option<ActivityCaptureScreenshotAnalysis>,
    pub last_screenshot_analysis_error: Option<ActivityCaptureScreenshotAnalysisError>,
    pub budget: ActivityCaptureBudget,
    pub analyze_screenshots: bool,
    pub screenshots_today: u32,
    pub screenshots_this_hour: u32,
    pub storage_used_mb: u64,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureBudget {
    pub min_interval_secs: u32,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureStateChanged {
    pub is_running: bool,
    pub changed_at_ms: i64,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureCapabilities {
    pub can_watch: bool,
    pub can_capture_visible_text: bool,
    pub can_capture_browser_url: bool,
    pub requires_accessibility_permission: bool,
}

#[derive(Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum ActivityCapturePluginEvent {
    #[serde(rename = "activityCaptureStateChanged")]
    StateChanged { state: ActivityCaptureStateChanged },
    #[serde(rename = "activityCaptureSignal")]
    Signal { signal: ActivityCaptureSignal },
    #[serde(rename = "activityCaptureError")]
    Error { error: ActivityCaptureRuntimeError },
    #[serde(rename = "activityCaptureScreenshotAnalysis")]
    ScreenshotAnalysis {
        analysis: ActivityCaptureScreenshotAnalysis,
    },
    #[serde(rename = "activityCaptureScreenshotAnalysisError")]
    ScreenshotAnalysisError {
        error: ActivityCaptureScreenshotAnalysisError,
    },
}

impl From<core::Capabilities> for ActivityCaptureCapabilities {
    fn from(value: core::Capabilities) -> Self {
        Self {
            can_watch: value.can_watch,
            can_capture_visible_text: value.can_capture_visible_text,
            can_capture_browser_url: value.can_capture_browser_url,
            requires_accessibility_permission: value.requires_accessibility_permission,
        }
    }
}

impl From<core::AppIdentity> for ActivityCaptureAppIdentity {
    fn from(value: core::AppIdentity) -> Self {
        Self {
            pid: value.pid,
            app_name: value.app_name,
            app_id: value.app_id,
            app_id_kind: value.app_id_kind,
            bundle_id: value.bundle_id,
            executable_path: value.executable_path,
        }
    }
}

impl From<core::Snapshot> for ActivityCaptureSnapshot {
    fn from(value: core::Snapshot) -> Self {
        Self {
            app: value.app.clone().into(),
            activity_kind: value.activity_kind,
            captured_at_ms: system_time_to_unix_ms(value.captured_at),
            pid: value.pid,
            app_name: value.app_name,
            bundle_id: value.bundle_id,
            focused_window_id: value.focused_window_id,
            window_title: value.window_title,
            url: value.url,
            visible_text: value.visible_text,
            text_anchor_kind: value.text_anchor_kind,
            text_anchor_identity: value.text_anchor_identity,
            text_anchor_text: value.text_anchor_text,
            text_anchor_prefix: value.text_anchor_prefix,
            text_anchor_suffix: value.text_anchor_suffix,
            text_anchor_selected_text: value.text_anchor_selected_text,
            text_anchor_confidence: value.text_anchor_confidence,
            content_level: value.content_level,
            source: value.source,
        }
    }
}

impl From<core::Transition> for ActivityCaptureSignal {
    fn from(value: core::Transition) -> Self {
        let occurred_at_ms = value
            .current
            .as_ref()
            .map(|event| system_time_to_unix_ms(event.started_at))
            .or_else(|| {
                value
                    .previous
                    .as_ref()
                    .map(|event| system_time_to_unix_ms(event.ended_at))
            })
            .unwrap_or_default();
        let fingerprint = value
            .current
            .as_ref()
            .map(|event| event.fingerprint.clone());
        Self {
            occurred_at_ms,
            reason: value.reason,
            sequence: value.sequence.min(i64::MAX as u64) as i64,
            suppressed_snapshot_count: value.suppressed_snapshot_count.min(i32::MAX as u32) as i32,
            fingerprint,
            snapshot: value.current.map(|event| event.snapshot.into()),
        }
    }
}

pub(crate) fn system_time_to_unix_ms(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}

pub(crate) fn unix_ms_now() -> i64 {
    system_time_to_unix_ms(SystemTime::now())
}
