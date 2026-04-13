use std::time::{SystemTime, UNIX_EPOCH};

use hypr_activity_capture as core;

pub use core::{
    ActivityKind, AppIdKind, CaptureErrorKind, ContentLevel, ObservationChangeClass,
    ObservationEndReason, ObservationEventKind, ObservationScreenshotKind, SnapshotSource,
    TextAnchorConfidence, TextAnchorKind,
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
pub struct ActivityCaptureObservation {
    pub observation_id: String,
    pub observation_key: String,
    pub started_at_ms: i64,
    pub last_seen_at_ms: i64,
    pub last_checkpoint_at_ms: Option<i64>,
    pub last_text_change_at_ms: Option<i64>,
    pub typing: bool,
    pub latest_snapshot: ActivityCaptureSnapshot,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureObservationEvent {
    pub id: String,
    pub observation_id: String,
    pub observation_key: String,
    pub kind: String,
    pub change_class: Option<String>,
    pub end_reason: Option<String>,
    pub occurred_at_ms: i64,
    pub started_at_ms: i64,
    pub snapshot: Option<ActivityCaptureSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureObservationAnalysis {
    pub observation_id: String,
    pub screenshot_id: String,
    pub screenshot_kind: String,
    pub captured_at_ms: i64,
    pub app_name: String,
    pub window_title: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureObservationAnalysisError {
    pub observation_id: String,
    pub screenshot_id: String,
    pub screenshot_kind: String,
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

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureConfig {
    pub poll_interval_ms: u64,
    pub entry_dwell_ms: u64,
    pub typing_settle_ms: u64,
    pub long_typing_checkpoint_ms: u64,
    pub refresh_interval_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCaptureStatus {
    pub is_running: bool,
    pub last_state_changed_at_ms: Option<i64>,
    pub current_observation: Option<ActivityCaptureObservation>,
    pub last_observation_event: Option<ActivityCaptureObservationEvent>,
    pub last_error: Option<ActivityCaptureRuntimeError>,
    pub last_observation_analysis: Option<ActivityCaptureObservationAnalysis>,
    pub last_observation_analysis_error: Option<ActivityCaptureObservationAnalysisError>,
    pub config: ActivityCaptureConfig,
    pub analyze_screenshots: bool,
    pub screenshots_today: u32,
    pub screenshots_this_hour: u32,
    pub storage_used_mb: u64,
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
    #[serde(rename = "activityObservationStarted")]
    ObservationStarted {
        event: ActivityCaptureObservationEvent,
    },
    #[serde(rename = "activityObservationCheckpointed")]
    ObservationCheckpointed {
        event: ActivityCaptureObservationEvent,
    },
    #[serde(rename = "activityObservationEnded")]
    ObservationEnded {
        event: ActivityCaptureObservationEvent,
    },
    #[serde(rename = "activityCaptureError")]
    Error { error: ActivityCaptureRuntimeError },
    #[serde(rename = "activityObservationAnalysisReady")]
    ObservationAnalysisReady {
        analysis: ActivityCaptureObservationAnalysis,
    },
    #[serde(rename = "activityObservationAnalysisError")]
    ObservationAnalysisError {
        error: ActivityCaptureObservationAnalysisError,
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

impl From<core::NormalizedSnapshot> for ActivityCaptureSnapshot {
    fn from(value: core::NormalizedSnapshot) -> Self {
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

impl From<core::ObservationState> for ActivityCaptureObservation {
    fn from(value: core::ObservationState) -> Self {
        Self {
            observation_id: value.observation_id,
            observation_key: value.observation_key,
            started_at_ms: system_time_to_unix_ms(value.started_at),
            last_seen_at_ms: system_time_to_unix_ms(value.last_seen_at),
            last_checkpoint_at_ms: value.last_checkpoint_at.map(system_time_to_unix_ms),
            last_text_change_at_ms: value.last_text_change_at.map(system_time_to_unix_ms),
            typing: value.typing,
            latest_snapshot: value.latest_snapshot.into(),
        }
    }
}

impl From<core::ObservationEvent> for ActivityCaptureObservationEvent {
    fn from(value: core::ObservationEvent) -> Self {
        Self {
            id: value.id,
            observation_id: value.observation_id,
            observation_key: value.observation_key,
            kind: value.kind.as_str().to_string(),
            change_class: value.change_class.map(|value| value.as_str().to_string()),
            end_reason: value.end_reason.map(|value| value.as_str().to_string()),
            occurred_at_ms: system_time_to_unix_ms(value.occurred_at),
            started_at_ms: system_time_to_unix_ms(value.started_at),
            snapshot: value.snapshot.map(Into::into),
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
