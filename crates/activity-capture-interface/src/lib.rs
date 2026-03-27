use std::{
    pin::Pin,
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use futures_core::Stream;

pub type CaptureStream =
    Pin<Box<dyn Stream<Item = Result<Transition, CaptureError>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    Accessibility,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub captured_at: SystemTime,
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: String,
    pub url: Option<String>,
    pub visible_text: String,
    pub source: SnapshotSource,
}

impl Snapshot {
    pub fn fingerprint(&self) -> String {
        STANDARD_NO_PAD.encode(
            [
                self.bundle_id.as_deref().unwrap_or_default(),
                &self.window_title,
                self.url.as_deref().unwrap_or_default(),
                &self.visible_text,
            ]
            .join("|"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub fingerprint: String,
    pub snapshot: Snapshot,
}

impl Event {
    pub fn from_snapshot(snapshot: Snapshot) -> Self {
        Self {
            started_at: snapshot.captured_at,
            ended_at: snapshot.captured_at,
            fingerprint: snapshot.fingerprint(),
            snapshot,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition {
    pub previous: Option<Event>,
    pub current: Option<Event>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Capabilities {
    pub can_watch: bool,
    pub can_capture_visible_text: bool,
    pub can_capture_browser_url: bool,
    pub requires_accessibility_permission: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    pub poll_interval: Duration,
    pub emit_initial: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            emit_initial: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureErrorKind {
    PermissionDenied,
    Unsupported,
    TemporarilyUnavailable,
    Platform,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct CaptureError {
    pub kind: CaptureErrorKind,
    pub message: String,
}

impl CaptureError {
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::PermissionDenied, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::Unsupported, message)
    }

    pub fn temporarily_unavailable(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::TemporarilyUnavailable, message)
    }

    pub fn platform(message: impl Into<String>) -> Self {
        Self::new(CaptureErrorKind::Platform, message)
    }

    pub fn new(kind: CaptureErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait ActivityCapture: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError>;

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError>;
}

#[derive(Debug, Default, Clone)]
pub struct EventCoalescer {
    current: Option<Event>,
}

impl EventCoalescer {
    pub fn current(&self) -> Option<&Event> {
        self.current.as_ref()
    }

    pub fn push(&mut self, snapshot: Option<Snapshot>) -> Option<Transition> {
        match (self.current.take(), snapshot) {
            (None, None) => None,
            (None, Some(snapshot)) => {
                let current = Event::from_snapshot(snapshot);
                self.current = Some(current.clone());
                Some(Transition {
                    previous: None,
                    current: Some(current),
                })
            }
            (Some(previous), None) => Some(Transition {
                previous: Some(previous),
                current: None,
            }),
            (Some(mut current), Some(snapshot)) => {
                let fingerprint = snapshot.fingerprint();
                if current.fingerprint == fingerprint {
                    current.ended_at = snapshot.captured_at;
                    current.snapshot = snapshot;
                    self.current = Some(current);
                    None
                } else {
                    let next = Event::from_snapshot(snapshot);
                    self.current = Some(next.clone());
                    Some(Transition {
                        previous: Some(current),
                        current: Some(next),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(title: &str) -> Snapshot {
        Snapshot {
            captured_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            pid: 42,
            app_name: "TextEdit".to_string(),
            bundle_id: Some("com.apple.TextEdit".to_string()),
            window_title: title.to_string(),
            url: None,
            visible_text: "hello".to_string(),
            source: SnapshotSource::Accessibility,
        }
    }

    #[test]
    fn fingerprint_is_stable() {
        let left = snapshot("Notes");
        let right = snapshot("Notes");

        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn coalescer_emits_initial_transition() {
        let mut coalescer = EventCoalescer::default();
        let transition = coalescer.push(Some(snapshot("Notes"))).unwrap();

        assert!(transition.previous.is_none());
        assert_eq!(transition.current.unwrap().snapshot.window_title, "Notes");
    }

    #[test]
    fn coalescer_suppresses_extensions() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));

        let mut same = snapshot("Notes");
        same.captured_at += Duration::from_secs(5);

        assert!(coalescer.push(Some(same)).is_none());
        assert_eq!(
            coalescer.current().unwrap().ended_at,
            SystemTime::UNIX_EPOCH + Duration::from_secs(15)
        );
    }

    #[test]
    fn coalescer_emits_change_transition() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));
        let transition = coalescer.push(Some(snapshot("Docs"))).unwrap();

        assert_eq!(transition.previous.unwrap().snapshot.window_title, "Notes");
        assert_eq!(transition.current.unwrap().snapshot.window_title, "Docs");
    }

    #[test]
    fn coalescer_emits_idle_transition() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));
        let transition = coalescer.push(None).unwrap();

        assert_eq!(transition.previous.unwrap().snapshot.window_title, "Notes");
        assert!(transition.current.is_none());
    }
}
