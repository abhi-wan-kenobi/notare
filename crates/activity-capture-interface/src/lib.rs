use std::{pin::Pin, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use futures_core::Stream;

mod types;

pub use types::*;

pub type CaptureStream =
    Pin<Box<dyn Stream<Item = Result<Transition, CaptureError>> + Send + 'static>>;

impl CaptureAccess {
    pub fn allows_snapshot(self) -> bool {
        self != Self::None
    }

    pub fn allows_url(self) -> bool {
        matches!(self, Self::Url | Self::Full)
    }

    pub fn allows_text(self) -> bool {
        self == Self::Full
    }
}

impl PolicyMode {
    pub fn default_access(self) -> CaptureAccess {
        match self {
            Self::OptIn => CaptureAccess::Metadata,
            Self::OptOut => CaptureAccess::Full,
        }
    }
}

impl DomainRule {
    pub fn matches_host(&self, host: &str) -> bool {
        let domain = normalize_domain(&self.domain);
        let host = normalize_domain(host);
        if domain.is_empty() || host.is_empty() {
            return false;
        }

        host == domain || (self.include_subdomains && host.ends_with(&format!(".{domain}")))
    }
}

impl BrowserPolicy {
    pub fn access_for_host(&self, host: Option<&str>, mode: PolicyMode) -> CaptureAccess {
        let Some(host) = host else {
            return mode.default_access();
        };

        let mut matched = None;
        for rule in &self.rules {
            if !rule.matches_host(host) {
                continue;
            }
            if rule.access == CaptureAccess::None {
                return CaptureAccess::None;
            }
            matched = Some(rule.access);
        }

        matched.unwrap_or_else(|| mode.default_access())
    }
}

impl Default for BrowserPolicy {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            require_url_for_text_access: true,
            block_private_browsing: true,
            strip_query: true,
            strip_fragment: true,
        }
    }
}

impl CapturePolicy {
    pub fn access_for_bundle(&self, bundle_id: Option<&str>) -> CaptureAccess {
        let Some(bundle_id) = bundle_id.map(str::trim).filter(|value| !value.is_empty()) else {
            return self.mode.default_access();
        };

        let mut matched = None;
        for rule in &self.app_rules {
            if rule.bundle_id != bundle_id {
                continue;
            }
            if rule.access == CaptureAccess::None {
                return CaptureAccess::None;
            }
            matched = Some(rule.access);
        }

        matched.unwrap_or_else(|| self.mode.default_access())
    }
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            mode: PolicyMode::OptIn,
            app_rules: Vec::new(),
            browser: BrowserPolicy::default(),
        }
    }
}

impl Snapshot {
    pub fn fingerprint(&self) -> String {
        let content_level = match self.content_level {
            ContentLevel::Metadata => "metadata",
            ContentLevel::Url => "url",
            ContentLevel::Full => "full",
        };
        let has_anchor = self.text_anchor_identity.is_some()
            || self.text_anchor_text.is_some()
            || self.text_anchor_selected_text.is_some();
        let ambient_text = if has_anchor {
            ""
        } else {
            self.visible_text.as_deref().unwrap_or_default()
        };

        STANDARD_NO_PAD.encode(
            [
                content_level,
                self.bundle_id.as_deref().unwrap_or_default(),
                self.window_title.as_deref().unwrap_or_default(),
                self.url.as_deref().unwrap_or_default(),
                self.text_anchor_identity.as_deref().unwrap_or_default(),
                self.text_anchor_text.as_deref().unwrap_or_default(),
                self.text_anchor_selected_text
                    .as_deref()
                    .unwrap_or_default(),
                ambient_text,
            ]
            .join("|"),
        )
    }
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

fn normalize_domain(value: &str) -> String {
    value.trim().trim_matches('.').to_ascii_lowercase()
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
    use std::time::SystemTime;

    fn snapshot(title: &str) -> Snapshot {
        Snapshot {
            captured_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            pid: 42,
            app_name: "TextEdit".to_string(),
            bundle_id: Some("com.apple.TextEdit".to_string()),
            window_title: Some(title.to_string()),
            url: None,
            visible_text: Some("hello".to_string()),
            text_anchor_kind: Some(TextAnchorKind::FocusedEdit),
            text_anchor_identity: Some("editor:notes".to_string()),
            text_anchor_text: Some("hello".to_string()),
            text_anchor_prefix: None,
            text_anchor_suffix: None,
            text_anchor_selected_text: None,
            text_anchor_confidence: Some(TextAnchorConfidence::High),
            content_level: ContentLevel::Full,
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
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Notes")
        );
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

        assert_eq!(
            transition
                .previous
                .unwrap()
                .snapshot
                .window_title
                .as_deref(),
            Some("Notes")
        );
        assert_eq!(
            transition.current.unwrap().snapshot.window_title.as_deref(),
            Some("Docs")
        );
    }

    #[test]
    fn coalescer_emits_idle_transition() {
        let mut coalescer = EventCoalescer::default();
        let _ = coalescer.push(Some(snapshot("Notes")));
        let transition = coalescer.push(None).unwrap();

        assert_eq!(
            transition
                .previous
                .unwrap()
                .snapshot
                .window_title
                .as_deref(),
            Some("Notes")
        );
        assert!(transition.current.is_none());
    }

    #[test]
    fn fingerprint_prefers_anchor_text_over_ambient_text() {
        let mut left = snapshot("Notes");
        let mut right = snapshot("Notes");

        left.visible_text = Some("ambient one".to_string());
        right.visible_text = Some("ambient two".to_string());

        assert_eq!(left.fingerprint(), right.fingerprint());

        right.text_anchor_text = Some("changed".to_string());

        assert_ne!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn fingerprint_falls_back_to_visible_text_when_anchor_is_missing() {
        let mut left = snapshot("Notes");
        let mut right = snapshot("Notes");

        left.text_anchor_kind = None;
        left.text_anchor_identity = None;
        left.text_anchor_text = None;
        left.text_anchor_prefix = None;
        left.text_anchor_suffix = None;
        left.text_anchor_selected_text = None;
        left.text_anchor_confidence = None;

        right.text_anchor_kind = None;
        right.text_anchor_identity = None;
        right.text_anchor_text = None;
        right.text_anchor_prefix = None;
        right.text_anchor_suffix = None;
        right.text_anchor_selected_text = None;
        right.text_anchor_confidence = None;
        right.visible_text = Some("different".to_string());

        assert_ne!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn domain_rule_matches_subdomains_when_enabled() {
        let rule = DomainRule {
            domain: "example.com".to_string(),
            include_subdomains: true,
            access: CaptureAccess::Full,
        };

        assert!(rule.matches_host("example.com"));
        assert!(rule.matches_host("docs.example.com"));
        assert!(!rule.matches_host("otherexample.com"));
    }

    #[test]
    fn browser_policy_uses_last_matching_rule() {
        let policy = BrowserPolicy {
            rules: vec![
                DomainRule {
                    domain: "example.com".to_string(),
                    include_subdomains: true,
                    access: CaptureAccess::Url,
                },
                DomainRule {
                    domain: "docs.example.com".to_string(),
                    include_subdomains: false,
                    access: CaptureAccess::Full,
                },
            ],
            ..Default::default()
        };

        assert_eq!(
            policy.access_for_host(Some("docs.example.com"), PolicyMode::OptIn),
            CaptureAccess::Full
        );
        assert_eq!(
            policy.access_for_host(Some("www.example.com"), PolicyMode::OptIn),
            CaptureAccess::Url
        );
        assert_eq!(
            policy.access_for_host(Some("other.com"), PolicyMode::OptIn),
            CaptureAccess::Metadata
        );
    }

    #[test]
    fn capture_policy_denies_when_any_matching_rule_denies() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptIn,
            app_rules: vec![
                BundleRule {
                    bundle_id: "com.example.app".to_string(),
                    access: CaptureAccess::None,
                },
                BundleRule {
                    bundle_id: "com.example.app".to_string(),
                    access: CaptureAccess::Full,
                },
            ],
            browser: BrowserPolicy::default(),
        };

        assert_eq!(
            policy.access_for_bundle(Some("com.example.app")),
            CaptureAccess::None
        );
        assert_eq!(
            policy.access_for_bundle(Some("com.example.other")),
            CaptureAccess::Metadata
        );
    }

    #[test]
    fn opt_out_policy_defaults_to_full() {
        let policy = CapturePolicy {
            mode: PolicyMode::OptOut,
            ..Default::default()
        };

        assert_eq!(
            policy.access_for_bundle(Some("com.example.app")),
            CaptureAccess::Full
        );
        assert_eq!(
            policy
                .browser
                .access_for_host(Some("example.com"), policy.mode),
            CaptureAccess::Full
        );
    }
}
