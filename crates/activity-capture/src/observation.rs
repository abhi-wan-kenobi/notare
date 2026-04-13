use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hypr_activity_capture_interface::NormalizedSnapshot;
use uuid::Uuid;

use crate::screenshot::{
    ObservationScreenshotKind, ObservationScreenshotRequest, target_from_snapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationEventKind {
    Started,
    Checkpointed,
    Ended,
}

impl ObservationEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Checkpointed => "checkpointed",
            Self::Ended => "ended",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "started" => Self::Started,
            "checkpointed" => Self::Checkpointed,
            "ended" => Self::Ended,
            _ => Self::Started,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationEndReason {
    Idle,
    Superseded,
}

impl ObservationEndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "idle" => Self::Idle,
            "superseded" => Self::Superseded,
            _ => Self::Idle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationChangeClass {
    Structural,
    Typing,
    Content,
}

impl ObservationChangeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Typing => "typing",
            Self::Content => "content",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "structural" => Self::Structural,
            "typing" => Self::Typing,
            "content" => Self::Content,
            _ => Self::Structural,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservationKey {
    pub app_id: String,
    pub activity_kind: String,
    pub container: String,
    pub text_anchor_identity: Option<String>,
}

impl ObservationKey {
    pub fn from_snapshot(snapshot: &NormalizedSnapshot) -> Self {
        Self {
            app_id: snapshot.app.app_id.clone(),
            activity_kind: snapshot.activity_kind.as_str().to_string(),
            container: container_identity(snapshot),
            text_anchor_identity: snapshot
                .text_anchor_identity
                .clone()
                .filter(|value| !value.is_empty()),
        }
    }

    pub fn as_string(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.app_id,
            self.activity_kind,
            self.container,
            self.text_anchor_identity.as_deref().unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservationState {
    pub observation_id: String,
    pub observation_key: String,
    pub started_at: SystemTime,
    pub last_seen_at: SystemTime,
    pub latest_snapshot: NormalizedSnapshot,
    pub last_checkpoint_at: Option<SystemTime>,
    pub last_text_change_at: Option<SystemTime>,
    pub typing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservationEvent {
    pub id: String,
    pub observation_id: String,
    pub observation_key: String,
    pub kind: ObservationEventKind,
    pub change_class: Option<ObservationChangeClass>,
    pub end_reason: Option<ObservationEndReason>,
    pub occurred_at: SystemTime,
    pub started_at: SystemTime,
    pub snapshot: Option<NormalizedSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservationUpdate {
    pub events: Vec<ObservationEvent>,
    pub capture_requests: Vec<ObservationScreenshotRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationReducerConfig {
    pub entry_dwell_ms: u64,
    pub typing_settle_ms: u64,
    pub long_typing_checkpoint_ms: u64,
    pub refresh_interval_ms: u64,
}

impl Default for ObservationReducerConfig {
    fn default() -> Self {
        Self {
            entry_dwell_ms: 1_200,
            typing_settle_ms: 2_500,
            long_typing_checkpoint_ms: 10_000,
            refresh_interval_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureSlot {
    Entry,
    Settled,
    Refresh,
}

#[derive(Debug, Clone)]
struct ActiveObservation {
    observation_id: String,
    observation_key: ObservationKey,
    started_at: SystemTime,
    last_seen_at: SystemTime,
    latest_snapshot: NormalizedSnapshot,
    last_checkpoint_at_ms: i64,
    last_text_change_at_ms: Option<i64>,
    last_checkpointed_text_change_at_ms: Option<i64>,
    last_screenshot_at_ms: Option<i64>,
    entry_due_at_ms: i64,
    entry_captured: bool,
    entry_request_id: Option<u64>,
    settled_due_at_ms: Option<i64>,
    settled_captured: bool,
    settled_request_id: Option<u64>,
    refresh_request_id: Option<u64>,
    typing: bool,
}

impl ActiveObservation {
    fn public_state(&self) -> ObservationState {
        ObservationState {
            observation_id: self.observation_id.clone(),
            observation_key: self.observation_key.as_string(),
            started_at: self.started_at,
            last_seen_at: self.last_seen_at,
            latest_snapshot: self.latest_snapshot.clone(),
            last_checkpoint_at: unix_ms_to_system_time(self.last_checkpoint_at_ms),
            last_text_change_at: self.last_text_change_at_ms.and_then(unix_ms_to_system_time),
            typing: self.typing,
        }
    }
}

#[derive(Debug, Default)]
pub struct ObservationReducer {
    config: ObservationReducerConfig,
    next_request_id: u64,
    active: Option<ActiveObservation>,
}

impl ObservationReducer {
    pub fn new(config: ObservationReducerConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn current_observation(&self) -> Option<ObservationState> {
        self.active.as_ref().map(ActiveObservation::public_state)
    }

    pub fn ingest(
        &mut self,
        captured_at: SystemTime,
        snapshot: Option<NormalizedSnapshot>,
    ) -> ObservationUpdate {
        let now_ms = system_time_to_unix_ms(captured_at);
        let mut update = ObservationUpdate::default();

        match (self.active.take(), snapshot) {
            (None, None) => {}
            (None, Some(snapshot)) => {
                let mut active = self.start_observation(snapshot, captured_at, now_ms, &mut update);
                self.collect_capture_requests(&mut active, now_ms, &mut update.capture_requests);
                self.active = Some(active);
            }
            (Some(active), None) => {
                self.end_observation(active, captured_at, ObservationEndReason::Idle, &mut update);
            }
            (Some(mut active), Some(snapshot)) => {
                let next_key = ObservationKey::from_snapshot(&snapshot);
                if active.observation_key != next_key {
                    self.end_observation(
                        active,
                        captured_at,
                        ObservationEndReason::Superseded,
                        &mut update,
                    );
                    let mut next =
                        self.start_observation(snapshot, captured_at, now_ms, &mut update);
                    self.collect_capture_requests(&mut next, now_ms, &mut update.capture_requests);
                    self.active = Some(next);
                } else {
                    self.update_active(&mut active, snapshot, captured_at, now_ms, &mut update);
                    self.collect_capture_requests(
                        &mut active,
                        now_ms,
                        &mut update.capture_requests,
                    );
                    self.active = Some(active);
                }
            }
        }

        update
    }

    pub fn acknowledge_capture(
        &mut self,
        request_id: u64,
        success: bool,
        captured_at_ms: i64,
    ) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };

        for slot in [
            CaptureSlot::Entry,
            CaptureSlot::Settled,
            CaptureSlot::Refresh,
        ] {
            let request_ref = match slot {
                CaptureSlot::Entry => &mut active.entry_request_id,
                CaptureSlot::Settled => &mut active.settled_request_id,
                CaptureSlot::Refresh => &mut active.refresh_request_id,
            };
            if *request_ref != Some(request_id) {
                continue;
            }

            *request_ref = None;
            if success {
                active.last_screenshot_at_ms = Some(captured_at_ms);
                match slot {
                    CaptureSlot::Entry => active.entry_captured = true,
                    CaptureSlot::Settled => active.settled_captured = true,
                    CaptureSlot::Refresh => {}
                }
            }
            return true;
        }

        false
    }

    fn start_observation(
        &mut self,
        snapshot: NormalizedSnapshot,
        captured_at: SystemTime,
        now_ms: i64,
        update: &mut ObservationUpdate,
    ) -> ActiveObservation {
        let observation_id = format!("obs-{}", Uuid::new_v4());
        let observation_key = ObservationKey::from_snapshot(&snapshot);
        let event = self.new_event(
            &observation_id,
            &observation_key,
            ObservationEventKind::Started,
            Some(ObservationChangeClass::Structural),
            None,
            captured_at,
            captured_at,
            Some(snapshot.clone()),
        );
        update.events.push(event);

        ActiveObservation {
            observation_id,
            observation_key,
            started_at: captured_at,
            last_seen_at: captured_at,
            latest_snapshot: snapshot,
            last_checkpoint_at_ms: now_ms,
            last_text_change_at_ms: None,
            last_checkpointed_text_change_at_ms: None,
            last_screenshot_at_ms: None,
            entry_due_at_ms: now_ms + self.config.entry_dwell_ms as i64,
            entry_captured: false,
            entry_request_id: None,
            settled_due_at_ms: None,
            settled_captured: false,
            settled_request_id: None,
            refresh_request_id: None,
            typing: false,
        }
    }

    fn update_active(
        &mut self,
        active: &mut ActiveObservation,
        snapshot: NormalizedSnapshot,
        captured_at: SystemTime,
        now_ms: i64,
        update: &mut ObservationUpdate,
    ) {
        let previous_snapshot = active.latest_snapshot.clone();
        let same_anchor = previous_snapshot.text_anchor_identity.is_some()
            && previous_snapshot.text_anchor_identity == snapshot.text_anchor_identity;
        let typing_change =
            same_anchor && previous_snapshot.primary_text() != snapshot.primary_text();
        let content_changed =
            previous_snapshot.content_fingerprint() != snapshot.content_fingerprint();

        active.last_seen_at = captured_at;
        active.latest_snapshot = snapshot.clone();

        if typing_change {
            active.last_text_change_at_ms = Some(now_ms);
            active.settled_due_at_ms = Some(now_ms + self.config.typing_settle_ms as i64);
            active.typing = true;
        }

        if active.typing {
            if let Some(due_at_ms) = active.settled_due_at_ms {
                if now_ms >= due_at_ms {
                    active.typing = false;
                }
            }
        }

        if let Some(last_text_change_at_ms) = active.last_text_change_at_ms {
            if now_ms - active.last_checkpoint_at_ms >= self.config.long_typing_checkpoint_ms as i64
                && last_text_change_at_ms > active.last_checkpoint_at_ms
            {
                update.events.push(self.new_event(
                    &active.observation_id,
                    &active.observation_key,
                    ObservationEventKind::Checkpointed,
                    Some(ObservationChangeClass::Typing),
                    None,
                    captured_at,
                    active.started_at,
                    Some(snapshot.clone()),
                ));
                active.last_checkpoint_at_ms = now_ms;
            }
        } else if content_changed {
            update.events.push(self.new_event(
                &active.observation_id,
                &active.observation_key,
                ObservationEventKind::Checkpointed,
                Some(ObservationChangeClass::Content),
                None,
                captured_at,
                active.started_at,
                Some(snapshot.clone()),
            ));
            active.last_checkpoint_at_ms = now_ms;
        }

        if let (Some(last_text_change_at_ms), Some(settled_due_at_ms)) =
            (active.last_text_change_at_ms, active.settled_due_at_ms)
        {
            if now_ms >= settled_due_at_ms
                && active.last_checkpointed_text_change_at_ms != Some(last_text_change_at_ms)
            {
                update.events.push(self.new_event(
                    &active.observation_id,
                    &active.observation_key,
                    ObservationEventKind::Checkpointed,
                    Some(ObservationChangeClass::Typing),
                    None,
                    captured_at,
                    active.started_at,
                    Some(snapshot),
                ));
                active.last_checkpoint_at_ms = now_ms;
                active.last_checkpointed_text_change_at_ms = Some(last_text_change_at_ms);
            }
        }
    }

    fn end_observation(
        &mut self,
        active: ActiveObservation,
        captured_at: SystemTime,
        reason: ObservationEndReason,
        update: &mut ObservationUpdate,
    ) {
        update.events.push(self.new_event(
            &active.observation_id,
            &active.observation_key,
            ObservationEventKind::Ended,
            None,
            Some(reason),
            captured_at,
            active.started_at,
            Some(active.latest_snapshot),
        ));
    }

    fn collect_capture_requests(
        &mut self,
        active: &mut ActiveObservation,
        now_ms: i64,
        requests: &mut Vec<ObservationScreenshotRequest>,
    ) {
        if !active.entry_captured
            && active.entry_request_id.is_none()
            && now_ms >= active.entry_due_at_ms
        {
            if let Some(target) = target_from_snapshot(&active.latest_snapshot) {
                let request_id = self.next_request_id();
                active.entry_request_id = Some(request_id);
                requests.push(ObservationScreenshotRequest {
                    request_id,
                    observation_id: active.observation_id.clone(),
                    observation_key: active.observation_key.as_string(),
                    kind: ObservationScreenshotKind::Entry,
                    scheduled_at_ms: active.started_at_ms(),
                    due_at_ms: active.entry_due_at_ms,
                    target,
                    snapshot: active.latest_snapshot.clone(),
                });
            }
        }

        if active.last_checkpointed_text_change_at_ms.is_some()
            && !active.settled_captured
            && active.settled_request_id.is_none()
        {
            if let Some(target) = target_from_snapshot(&active.latest_snapshot) {
                let request_id = self.next_request_id();
                active.settled_request_id = Some(request_id);
                requests.push(ObservationScreenshotRequest {
                    request_id,
                    observation_id: active.observation_id.clone(),
                    observation_key: active.observation_key.as_string(),
                    kind: ObservationScreenshotKind::Settled,
                    scheduled_at_ms: active
                        .last_text_change_at_ms
                        .unwrap_or_else(|| active.started_at_ms()),
                    due_at_ms: active.settled_due_at_ms.unwrap_or(now_ms),
                    target,
                    snapshot: active.latest_snapshot.clone(),
                });
            }
        }

        if active.last_screenshot_at_ms.is_some()
            && active.refresh_request_id.is_none()
            && now_ms - active.last_screenshot_at_ms.unwrap_or_default()
                >= self.config.refresh_interval_ms as i64
        {
            if let Some(target) = target_from_snapshot(&active.latest_snapshot) {
                let request_id = self.next_request_id();
                active.refresh_request_id = Some(request_id);
                requests.push(ObservationScreenshotRequest {
                    request_id,
                    observation_id: active.observation_id.clone(),
                    observation_key: active.observation_key.as_string(),
                    kind: ObservationScreenshotKind::Refresh,
                    scheduled_at_ms: active.last_screenshot_at_ms.unwrap_or_default(),
                    due_at_ms: now_ms,
                    target,
                    snapshot: active.latest_snapshot.clone(),
                });
            }
        }
    }

    fn new_event(
        &mut self,
        observation_id: &str,
        observation_key: &ObservationKey,
        kind: ObservationEventKind,
        change_class: Option<ObservationChangeClass>,
        end_reason: Option<ObservationEndReason>,
        occurred_at: SystemTime,
        started_at: SystemTime,
        snapshot: Option<NormalizedSnapshot>,
    ) -> ObservationEvent {
        ObservationEvent {
            id: format!("obs_evt-{}", Uuid::new_v4()),
            observation_id: observation_id.to_string(),
            observation_key: observation_key.as_string(),
            kind,
            change_class,
            end_reason,
            occurred_at,
            started_at,
            snapshot,
        }
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_request_id += 1;
        self.next_request_id
    }
}

impl ActiveObservation {
    fn started_at_ms(&self) -> i64 {
        system_time_to_unix_ms(self.started_at)
    }
}

fn container_identity(snapshot: &NormalizedSnapshot) -> String {
    if let Some(window_id) = snapshot.focused_window_id {
        return format!("window:{window_id}");
    }
    if let Some(url) = snapshot.url.as_deref().filter(|value| !value.is_empty()) {
        return format!("url:{url}");
    }
    if let Some(title) = snapshot
        .window_title
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return format!("title:{title}");
    }
    "none".to_string()
}

pub fn system_time_to_unix_ms(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}

fn unix_ms_to_system_time(value: i64) -> Option<SystemTime> {
    if value == 0 {
        return None;
    }
    if value >= 0 {
        Some(UNIX_EPOCH + Duration::from_millis(value as u64))
    } else {
        Some(UNIX_EPOCH - Duration::from_millis(value.unsigned_abs()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hypr_activity_capture_interface::{
        ActivityKind, AppIdKind, AppIdentity, ContentLevel, NormalizedSnapshot, SnapshotSource,
        TextAnchorConfidence, TextAnchorKind,
    };

    use super::{ObservationEventKind, ObservationReducer, ObservationReducerConfig};

    fn snapshot(anchor: &str, text: &str) -> NormalizedSnapshot {
        NormalizedSnapshot {
            app: AppIdentity {
                pid: 42,
                app_name: "Mail".to_string(),
                app_id: "com.apple.mail".to_string(),
                app_id_kind: AppIdKind::BundleId,
                bundle_id: Some("com.apple.mail".to_string()),
                executable_path: None,
            },
            activity_kind: ActivityKind::ForegroundWindow,
            captured_at: std::time::SystemTime::UNIX_EPOCH,
            pid: 42,
            app_name: "Mail".to_string(),
            bundle_id: Some("com.apple.mail".to_string()),
            focused_window_id: Some(100),
            window_title: Some("Compose".to_string()),
            url: None,
            visible_text: Some(text.to_string()),
            text_anchor_kind: Some(TextAnchorKind::FocusedEdit),
            text_anchor_identity: Some(anchor.to_string()),
            text_anchor_text: Some(text.to_string()),
            text_anchor_prefix: None,
            text_anchor_suffix: None,
            text_anchor_selected_text: None,
            text_anchor_confidence: Some(TextAnchorConfidence::High),
            content_level: ContentLevel::Full,
            source: SnapshotSource::Accessibility,
        }
    }

    #[test]
    fn typing_in_same_anchor_keeps_one_observation() {
        let mut reducer = ObservationReducer::new(ObservationReducerConfig::default());
        let first = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(0),
            Some(snapshot("compose:body", "h")),
        );
        let second = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(500),
            Some(snapshot("compose:body", "he")),
        );

        assert_eq!(first.events.len(), 1);
        assert_eq!(first.events[0].kind, ObservationEventKind::Started);
        assert!(second.events.is_empty());
        assert_eq!(
            reducer.current_observation().unwrap().observation_id,
            first.events[0].observation_id
        );
    }

    #[test]
    fn changing_anchor_supersedes_observation() {
        let mut reducer = ObservationReducer::new(ObservationReducerConfig::default());
        let first = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH,
            Some(snapshot("compose:to", "a")),
        );
        let second = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(1000),
            Some(snapshot("compose:body", "hello")),
        );

        assert_eq!(first.events.len(), 1);
        assert_eq!(second.events.len(), 2);
        assert_eq!(second.events[0].kind, ObservationEventKind::Ended);
        assert_eq!(second.events[1].kind, ObservationEventKind::Started);
    }

    #[test]
    fn settled_typing_emits_checkpoint_and_request() {
        let mut reducer = ObservationReducer::new(ObservationReducerConfig::default());
        let _ = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH,
            Some(snapshot("compose:body", "h")),
        );
        let _ = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(500),
            Some(snapshot("compose:body", "he")),
        );
        let settled = reducer.ingest(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_millis(3200),
            Some(snapshot("compose:body", "he")),
        );

        assert!(
            settled
                .events
                .iter()
                .any(|event| event.kind == ObservationEventKind::Checkpointed)
        );
        assert!(
            settled
                .capture_requests
                .iter()
                .any(|request| request.kind == crate::ObservationScreenshotKind::Settled)
        );
    }

    #[test]
    fn reducer_restarts_do_not_reuse_persisted_ids() {
        let first = ObservationReducer::new(ObservationReducerConfig::default()).ingest(
            std::time::SystemTime::UNIX_EPOCH,
            Some(snapshot("compose:body", "hello")),
        );
        let second = ObservationReducer::new(ObservationReducerConfig::default()).ingest(
            std::time::SystemTime::UNIX_EPOCH,
            Some(snapshot("compose:body", "hello")),
        );

        assert_eq!(first.events.len(), 1);
        assert_eq!(second.events.len(), 1);
        assert_ne!(first.events[0].id, second.events[0].id);
        assert_ne!(
            first.events[0].observation_id,
            second.events[0].observation_id
        );
        assert!(first.events[0].id.starts_with("obs_evt-"));
        assert!(first.events[0].observation_id.starts_with("obs-"));
    }
}
