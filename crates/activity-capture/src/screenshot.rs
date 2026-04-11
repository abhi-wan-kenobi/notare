use std::collections::HashMap;

use hypr_activity_capture_interface::{ActivityKind, Snapshot, Transition, TransitionReason};
use hypr_screen_core::{WindowCaptureTarget, WindowContextCaptureOptions, WindowContextImage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityScreenshotTarget {
    pub window_id: Option<u32>,
    pub pid: u32,
    pub app_name: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityScreenshotCapture {
    pub fingerprint: String,
    pub reason: TransitionReason,
    pub scheduled_at_ms: i64,
    pub captured_at_ms: i64,
    pub target: ActivityScreenshotTarget,
    pub image: hypr_screen_core::WindowContextImage,
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
    pub target: ActivityScreenshotTarget,
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
    last_capture_ms_by_scope: HashMap<String, i64>,
}

impl ScreenshotPolicy {
    pub fn new(config: ScreenshotConfig) -> Self {
        Self {
            config,
            next_id: 0,
            pending: None,
            last_capture_ms_by_scope: HashMap::new(),
        }
    }

    pub fn config(&self) -> &ScreenshotConfig {
        &self.config
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
        if !is_supported_kind(snapshot.activity_kind) {
            return self.clear_pending();
        }

        if !is_focus_trigger(transition.reason) {
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

        let target = match target_from_snapshot(snapshot) {
            Some(t) => t,
            None => return self.clear_pending(),
        };

        self.next_id += 1;
        let dwell_ms = self.config.dwell_ms.min(i64::MAX as u64) as i64;
        let pending = PendingCapture {
            id: self.next_id,
            fingerprint: current.fingerprint.clone(),
            reason: transition.reason,
            cooldown_scope,
            scheduled_at_ms: now_ms,
            due_at_ms: now_ms.saturating_add(dwell_ms),
            target,
        };

        self.replace_pending(pending)
    }

    pub fn fire(&mut self, pending_id: u64, now_ms: i64) -> Option<PendingCapture> {
        let pending = match self.pending.as_ref() {
            Some(p) if p.id == pending_id && now_ms >= p.due_at_ms => p.clone(),
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

fn is_focus_trigger(reason: TransitionReason) -> bool {
    matches!(
        reason,
        TransitionReason::Started | TransitionReason::AppChanged | TransitionReason::WindowChanged
    )
}

fn is_supported_kind(kind: ActivityKind) -> bool {
    matches!(kind, ActivityKind::ForegroundWindow | ActivityKind::Browser)
}

pub fn capture_screenshot(target: &ActivityScreenshotTarget) -> Result<WindowContextImage, String> {
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

fn target_from_snapshot(snapshot: &Snapshot) -> Option<ActivityScreenshotTarget> {
    Some(ActivityScreenshotTarget {
        window_id: snapshot.focused_window_id,
        pid: u32::try_from(snapshot.pid).ok()?,
        app_name: snapshot.app_name.clone(),
        title: snapshot
            .window_title
            .clone()
            .filter(|value| !value.is_empty()),
    })
}

fn is_excluded_snapshot(snapshot: &Snapshot, excluded_app_ids: &[String]) -> bool {
    if excluded_app_ids.is_empty() {
        return false;
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_activity_capture_interface::{
        AppIdKind, AppIdentity, CaptureAccess, Event, SnapshotSource, SnapshotSpec,
    };
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn snapshot(kind: ActivityKind, pid: i32, title: &str) -> Snapshot {
        Snapshot::from_spec(SnapshotSpec {
            captured_at: UNIX_EPOCH + Duration::from_secs(1),
            app: AppIdentity {
                pid,
                app_name: "Code".to_string(),
                app_id: "com.microsoft.VSCode".to_string(),
                app_id_kind: AppIdKind::BundleId,
                bundle_id: Some("com.microsoft.VSCode".to_string()),
                executable_path: None,
            },
            activity_kind: kind,
            access: CaptureAccess::Full,
            source: SnapshotSource::Accessibility,
            focused_window_id: Some(101),
            window_title: Some(title.to_string()),
            url: None,
            visible_text: None,
            text_anchor: None,
        })
    }

    fn transition(reason: TransitionReason, fingerprint: &str, snapshot: Snapshot) -> Transition {
        Transition {
            previous: None,
            current: Some(Event {
                started_at: SystemTime::UNIX_EPOCH,
                ended_at: SystemTime::UNIX_EPOCH,
                fingerprint: fingerprint.to_string(),
                snapshot,
            }),
            reason,
            sequence: 1,
            suppressed_snapshot_count: 0,
        }
    }

    fn snapshot_with_bundle_id(
        kind: ActivityKind,
        pid: i32,
        title: &str,
        bundle_id: &str,
    ) -> Snapshot {
        let mut snapshot = snapshot(kind, pid, title);
        snapshot.app.bundle_id = Some(bundle_id.to_string());
        snapshot.app.app_id = bundle_id.to_string();
        snapshot
    }

    fn snapshot_with_app_id(kind: ActivityKind, pid: i32, title: &str, app_id: &str) -> Snapshot {
        let mut snapshot = snapshot(kind, pid, title);
        snapshot.app.bundle_id = None;
        snapshot.app.app_id = app_id.to_string();
        snapshot
    }

    fn snapshot_with_executable_path(
        kind: ActivityKind,
        pid: i32,
        title: &str,
        executable_path: &str,
    ) -> Snapshot {
        let mut snapshot = snapshot(kind, pid, title);
        snapshot.app.executable_path = Some(executable_path.to_string());
        snapshot.app.app_id = executable_path.to_string();
        snapshot
    }

    fn idle_transition() -> Transition {
        Transition {
            previous: None,
            current: None,
            reason: TransitionReason::Idle,
            sequence: 2,
            suppressed_snapshot_count: 0,
        }
    }

    #[test]
    fn schedules_on_started() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let decision = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );

        match decision {
            ScreenshotDecision::Schedule(pending) => {
                assert_eq!(pending.fingerprint, "fp1");
                assert_eq!(pending.due_at_ms, 3_000);
            }
            other => panic!("expected Schedule, got {other:?}"),
        }
    }

    #[test]
    fn idle_clears_pending() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let _ = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );
        let decision = policy.on_transition(&idle_transition(), 2_000);
        assert_eq!(decision, ScreenshotDecision::CancelPending);
    }

    #[test]
    fn excludes_configured_app_ids() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: ScreenshotConfig::default().dwell_ms,
            min_interval_secs: ScreenshotConfig::default().min_interval_secs,
            excluded_app_ids: vec!["com.hyprnote.stable".to_string()],
        });

        let decision = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp-self",
                snapshot_with_bundle_id(
                    ActivityKind::ForegroundWindow,
                    42,
                    "main.rs",
                    "com.hyprnote.stable",
                ),
            ),
            1_000,
        );

        assert_eq!(decision, ScreenshotDecision::None);
    }

    #[test]
    fn excludes_matching_executable_paths() {
        let executable_path = "/Applications/Char.app/Contents/MacOS/Char";
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: ScreenshotConfig::default().dwell_ms,
            min_interval_secs: ScreenshotConfig::default().min_interval_secs,
            excluded_app_ids: vec![executable_path.to_string()],
        });

        let decision = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp-self-exe",
                snapshot_with_executable_path(
                    ActivityKind::ForegroundWindow,
                    42,
                    "main.rs",
                    executable_path,
                ),
            ),
            1_000,
        );

        assert_eq!(decision, ScreenshotDecision::None);
    }

    #[test]
    fn unrelated_app_still_schedules_when_exclusions_exist() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: ScreenshotConfig::default().dwell_ms,
            min_interval_secs: ScreenshotConfig::default().min_interval_secs,
            excluded_app_ids: vec!["com.hyprnote.stable".to_string()],
        });

        let decision = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp-other",
                snapshot_with_bundle_id(
                    ActivityKind::ForegroundWindow,
                    42,
                    "main.rs",
                    "com.microsoft.VSCode",
                ),
            ),
            1_000,
        );

        match decision {
            ScreenshotDecision::Schedule(pending) => {
                assert_eq!(pending.fingerprint, "fp-other");
            }
            other => panic!("expected Schedule, got {other:?}"),
        }
    }

    #[test]
    fn respects_min_interval_within_same_app_scope() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: 0,
            min_interval_secs: 6,
            excluded_app_ids: Vec::new(),
        });

        let p = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            ScreenshotDecision::Schedule(p) => p,
            other => panic!("expected Schedule, got {other:?}"),
        };
        assert!(policy.fire(p.id, 1_000).is_some());

        let decision = policy.on_transition(
            &transition(
                TransitionReason::AppChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 43, "lib.rs"),
            ),
            2_000,
        );
        assert_eq!(decision, ScreenshotDecision::None);
    }

    #[test]
    fn min_interval_does_not_block_different_app_scope() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: 0,
            min_interval_secs: 6,
            excluded_app_ids: Vec::new(),
        });

        let p = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot_with_bundle_id(
                    ActivityKind::ForegroundWindow,
                    42,
                    "main.rs",
                    "com.microsoft.VSCode",
                ),
            ),
            1_000,
        ) {
            ScreenshotDecision::Schedule(p) => p,
            other => panic!("expected Schedule, got {other:?}"),
        };
        assert!(policy.fire(p.id, 1_000).is_some());

        let decision = policy.on_transition(
            &transition(
                TransitionReason::AppChanged,
                "fp2",
                snapshot_with_bundle_id(
                    ActivityKind::ForegroundWindow,
                    43,
                    "KakaoTalk",
                    "com.kakao.KakaoTalkMac",
                ),
            ),
            2_000,
        );

        match decision {
            ScreenshotDecision::Schedule(pending) => {
                assert_eq!(pending.cooldown_scope, "com.kakao.KakaoTalkMac");
            }
            other => panic!("expected Schedule, got {other:?}"),
        }
    }

    #[test]
    fn min_interval_falls_back_to_app_id_when_bundle_id_is_missing() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig {
            dwell_ms: 0,
            min_interval_secs: 6,
            excluded_app_ids: Vec::new(),
        });

        let p = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot_with_app_id(ActivityKind::ForegroundWindow, 42, "Draft", "pid:42"),
            ),
            1_000,
        ) {
            ScreenshotDecision::Schedule(p) => p,
            other => panic!("expected Schedule, got {other:?}"),
        };
        assert!(policy.fire(p.id, 1_000).is_some());

        let decision = policy.on_transition(
            &transition(
                TransitionReason::AppChanged,
                "fp2",
                snapshot_with_app_id(ActivityKind::ForegroundWindow, 42, "Draft 2", "pid:42"),
            ),
            2_000,
        );

        assert_eq!(decision, ScreenshotDecision::None);
    }

    #[test]
    fn fire_returns_none_before_due() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let p = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            ScreenshotDecision::Schedule(p) => p,
            other => panic!("expected Schedule, got {other:?}"),
        };

        assert!(policy.fire(p.id, 2_999).is_none());
        assert!(policy.fire(p.id, 3_000).is_some());
    }

    #[test]
    fn title_changed_does_not_schedule_without_pending() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let decision = policy.on_transition(
            &transition(
                TransitionReason::TitleChanged,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "renamed.rs"),
            ),
            1_000,
        );

        assert_eq!(decision, ScreenshotDecision::None);
    }

    #[test]
    fn content_updates_do_not_cancel_pending_focus_capture() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let pending = match policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        ) {
            ScreenshotDecision::Schedule(pending) => pending,
            other => panic!("expected Schedule, got {other:?}"),
        };

        let decision = policy.on_transition(
            &transition(
                TransitionReason::TitleChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs (edited)"),
            ),
            2_000,
        );

        assert_eq!(decision, ScreenshotDecision::None);
        assert!(policy.fire(pending.id, pending.due_at_ms).is_some());
    }

    #[test]
    fn app_changed_replaces_pending_capture() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let _ = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );

        let decision = policy.on_transition(
            &transition(
                TransitionReason::AppChanged,
                "fp2",
                snapshot(ActivityKind::ForegroundWindow, 43, "Inbox"),
            ),
            2_000,
        );

        match decision {
            ScreenshotDecision::CancelAndSchedule(pending) => {
                assert_eq!(pending.fingerprint, "fp2");
                assert_eq!(pending.target.pid, 43);
            }
            other => panic!("expected CancelAndSchedule, got {other:?}"),
        }
    }

    #[test]
    fn window_changed_replaces_pending_capture() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let _ = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::ForegroundWindow, 42, "main.rs"),
            ),
            1_000,
        );

        let mut next = snapshot(ActivityKind::ForegroundWindow, 42, "Inbox");
        next.focused_window_id = Some(202);
        let decision = policy.on_transition(
            &transition(TransitionReason::WindowChanged, "fp2", next),
            2_000,
        );

        match decision {
            ScreenshotDecision::CancelAndSchedule(pending) => {
                assert_eq!(pending.fingerprint, "fp2");
                assert_eq!(pending.target.window_id, Some(202));
            }
            other => panic!("expected CancelAndSchedule, got {other:?}"),
        }
    }

    #[test]
    fn ignores_audio_session() {
        let mut policy = ScreenshotPolicy::new(ScreenshotConfig::default());
        let decision = policy.on_transition(
            &transition(
                TransitionReason::Started,
                "fp1",
                snapshot(ActivityKind::AudioSession, 42, "main.rs"),
            ),
            1_000,
        );
        assert_eq!(decision, ScreenshotDecision::None);
    }
}
