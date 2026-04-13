use std::{
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use futures_core::Stream;
use url::{Host, Url};

mod types;

pub use types::*;

pub type Snapshot = NormalizedSnapshot;
pub type SnapshotSpec = NormalizedSnapshotSpec;

pub type CaptureStream =
    Pin<Box<dyn Stream<Item = Result<RawCaptureSample, CaptureError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub started_at: std::time::SystemTime,
    pub ended_at: std::time::SystemTime,
    pub fingerprint: String,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionReason {
    Started,
    Idle,
    AppChanged,
    WindowChanged,
    ActivityKindChanged,
    UrlChanged,
    TitleChanged,
    TextAnchorChanged,
    ContentChanged,
}

impl TransitionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Idle => "idle",
            Self::AppChanged => "app_changed",
            Self::WindowChanged => "window_changed",
            Self::ActivityKindChanged => "activity_kind_changed",
            Self::UrlChanged => "url_changed",
            Self::TitleChanged => "title_changed",
            Self::TextAnchorChanged => "text_anchor_changed",
            Self::ContentChanged => "content_changed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "started" => Self::Started,
            "idle" => Self::Idle,
            "app_changed" => Self::AppChanged,
            "window_changed" => Self::WindowChanged,
            "activity_kind_changed" => Self::ActivityKindChanged,
            "url_changed" => Self::UrlChanged,
            "title_changed" => Self::TitleChanged,
            "text_anchor_changed" => Self::TextAnchorChanged,
            "content_changed" => Self::ContentChanged,
            _ => Self::Started,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition {
    pub previous: Option<Event>,
    pub current: Option<Event>,
    pub reason: TransitionReason,
    pub sequence: u64,
    pub suppressed_snapshot_count: u32,
}

#[derive(Debug, Default, Clone)]
pub struct EventCoalescer {
    current: Option<Event>,
    current_suppressed_snapshot_count: u32,
    sequence: u64,
}

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

    pub fn sanitize_url(&self, raw_url: &str) -> Option<SanitizedBrowserUrl> {
        let mut parsed = Url::parse(raw_url).ok()?;
        let host = parsed
            .host()
            .and_then(normalize_host)
            .map(normalize_domain)
            .filter(|value| !value.is_empty());

        if self.strip_query {
            parsed.set_query(None);
        }
        if self.strip_fragment {
            parsed.set_fragment(None);
        }

        Some(SanitizedBrowserUrl {
            url: parsed.to_string(),
            host,
        })
    }

    pub fn access_for_context(
        &self,
        browser: &BrowserContext,
        mode: PolicyMode,
    ) -> CaptureDecision {
        if self.block_private_browsing && browser.is_private {
            return CaptureDecision {
                access: CaptureAccess::None,
                skip: true,
                source: SnapshotSource::Workspace,
                activity_kind: ActivityKind::Browser,
                url: None,
            };
        }

        let sanitized = browser
            .raw_url
            .as_deref()
            .and_then(|value| self.sanitize_url(value));
        let has_sanitized_url = sanitized.is_some();
        let mut access = self.access_for_host(
            sanitized.as_ref().and_then(|value| value.host.as_deref()),
            mode,
        );
        if !has_sanitized_url && access > CaptureAccess::Metadata {
            access = CaptureAccess::Metadata;
        }
        if !has_sanitized_url && access.allows_text() && self.require_url_for_text_access {
            access = CaptureAccess::Metadata;
        }

        CaptureDecision {
            access,
            skip: !access.allows_snapshot(),
            source: source_for_access(access, SnapshotSource::Accessibility),
            activity_kind: ActivityKind::Browser,
            url: access
                .allows_url()
                .then(|| sanitized.map(|value| value.url))
                .flatten(),
        }
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
    pub fn access_for_app(&self, app: &AppIdentity) -> CaptureAccess {
        let ids = [
            app.bundle_id.as_deref(),
            Some(app.app_id.as_str()),
            app.executable_path.as_deref(),
        ];

        let mut matched = None;
        for candidate in ids.into_iter().flatten() {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                continue;
            }

            for rule in &self.app_rules {
                if rule.bundle_id != candidate {
                    continue;
                }
                if rule.access == CaptureAccess::None {
                    return CaptureAccess::None;
                }
                matched = Some(rule.access);
            }
        }

        matched.unwrap_or_else(|| self.mode.default_access())
    }

    pub fn decision_for_candidate(&self, candidate: &CaptureCandidate) -> CaptureDecision {
        let app_access = self.access_for_app(&candidate.app);
        if !app_access.allows_snapshot() {
            return CaptureDecision {
                access: app_access,
                skip: true,
                source: SnapshotSource::Workspace,
                activity_kind: candidate.activity_kind,
                url: None,
            };
        }

        if let Some(browser) = &candidate.browser {
            let browser_decision = self.browser.access_for_context(browser, self.mode);
            let access = std::cmp::min(app_access, browser_decision.access);
            return CaptureDecision {
                access,
                skip: !access.allows_snapshot() || browser_decision.skip,
                source: source_for_access(access, candidate.source),
                activity_kind: ActivityKind::Browser,
                url: access
                    .allows_url()
                    .then_some(browser_decision.url)
                    .flatten(),
            };
        }

        let access = normalize_non_browser_access(app_access);
        CaptureDecision {
            access,
            skip: !access.allows_snapshot(),
            source: source_for_access(access, candidate.source),
            activity_kind: candidate.activity_kind,
            url: None,
        }
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

impl NormalizedSnapshot {
    pub fn from_spec(spec: NormalizedSnapshotSpec) -> Self {
        let content_level = content_level_for_access(spec.access);
        let text_anchor = spec
            .access
            .allows_text()
            .then_some(spec.text_anchor)
            .flatten();

        Self {
            app: spec.app.clone(),
            activity_kind: spec.activity_kind,
            captured_at: spec.captured_at,
            pid: spec.app.pid,
            app_name: spec.app.app_name.clone(),
            bundle_id: spec.app.bundle_id.clone(),
            focused_window_id: spec.focused_window_id,
            window_title: spec
                .access
                .allows_text()
                .then_some(spec.window_title)
                .flatten(),
            url: spec.access.allows_url().then_some(spec.url).flatten(),
            visible_text: spec
                .access
                .allows_text()
                .then_some(spec.visible_text)
                .flatten(),
            text_anchor_kind: text_anchor.as_ref().map(|value| value.kind),
            text_anchor_identity: text_anchor.as_ref().map(|value| value.identity.clone()),
            text_anchor_text: text_anchor.as_ref().and_then(|value| value.text.clone()),
            text_anchor_prefix: text_anchor.as_ref().and_then(|value| value.prefix.clone()),
            text_anchor_suffix: text_anchor.as_ref().and_then(|value| value.suffix.clone()),
            text_anchor_selected_text: text_anchor
                .as_ref()
                .and_then(|value| value.selected_text.clone()),
            text_anchor_confidence: text_anchor.as_ref().map(|value| value.confidence),
            content_level,
            source: source_for_access(spec.access, spec.source),
        }
    }

    pub fn primary_text(&self) -> Option<&str> {
        self.text_anchor_text
            .as_deref()
            .or(self.text_anchor_selected_text.as_deref())
            .or(self.visible_text.as_deref())
            .filter(|value| !value.is_empty())
    }

    pub fn content_fingerprint(&self) -> String {
        let ambient_text = if self.text_anchor_identity.is_some() {
            ""
        } else {
            self.visible_text.as_deref().unwrap_or_default()
        };

        STANDARD_NO_PAD.encode(
            [
                self.content_level.as_str(),
                self.app.app_id.as_str(),
                self.activity_kind.as_str(),
                &self
                    .focused_window_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
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
            fingerprint: snapshot.content_fingerprint(),
            snapshot,
        }
    }
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
                self.current_suppressed_snapshot_count = 0;
                self.sequence += 1;
                Some(Transition {
                    previous: None,
                    current: Some(current),
                    reason: TransitionReason::Started,
                    sequence: self.sequence,
                    suppressed_snapshot_count: 0,
                })
            }
            (Some(previous), None) => {
                let suppressed_snapshot_count = self.current_suppressed_snapshot_count;
                self.current_suppressed_snapshot_count = 0;
                self.sequence += 1;
                Some(Transition {
                    previous: Some(previous),
                    current: None,
                    reason: TransitionReason::Idle,
                    sequence: self.sequence,
                    suppressed_snapshot_count,
                })
            }
            (Some(mut current), Some(snapshot)) => {
                let fingerprint = snapshot.content_fingerprint();
                if current.fingerprint == fingerprint {
                    current.ended_at = snapshot.captured_at;
                    current.snapshot = snapshot;
                    self.current = Some(current);
                    self.current_suppressed_snapshot_count += 1;
                    None
                } else {
                    let next = Event::from_snapshot(snapshot);
                    let reason = transition_reason(&current.snapshot, &next.snapshot);
                    let suppressed_snapshot_count = self.current_suppressed_snapshot_count;
                    self.current = Some(next.clone());
                    self.current_suppressed_snapshot_count = 0;
                    self.sequence += 1;
                    Some(Transition {
                        previous: Some(current),
                        current: Some(next),
                        reason,
                        sequence: self.sequence,
                        suppressed_snapshot_count,
                    })
                }
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
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

    fn snapshot(&self) -> Result<Option<NormalizedSnapshot>, CaptureError>;

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError>;
}

pub fn spawn_polling_watch_stream<F>(
    thread_name: impl Into<String>,
    poll_snapshot: F,
    options: WatchOptions,
) -> Result<CaptureStream, CaptureError>
where
    F: FnMut() -> Result<Option<NormalizedSnapshot>, CaptureError> + Send + 'static,
{
    let (sample_tx, sample_rx) = tokio::sync::mpsc::unbounded_channel();
    let stop = Arc::new(StopSignal::default());
    let thread_stop = Arc::clone(&stop);

    let handle = thread::Builder::new()
        .name(thread_name.into())
        .spawn(move || watch_loop(poll_snapshot, options, thread_stop, sample_tx))
        .map_err(|error| CaptureError::platform(error.to_string()))?;

    Ok(Box::pin(WatchStream {
        inner: tokio_stream::wrappers::UnboundedReceiverStream::new(sample_rx),
        stop,
        handle: Some(handle),
    }))
}

fn normalize_domain(value: &str) -> String {
    value.trim().trim_matches('.').to_ascii_lowercase()
}

fn normalize_host(host: Host<&str>) -> Option<&str> {
    match host {
        Host::Domain(value) => Some(value),
        _ => None,
    }
}

fn normalize_non_browser_access(access: CaptureAccess) -> CaptureAccess {
    match access {
        CaptureAccess::Url => CaptureAccess::Metadata,
        _ => access,
    }
}

pub fn content_level_for_access(access: CaptureAccess) -> ContentLevel {
    match access {
        CaptureAccess::Metadata | CaptureAccess::None => ContentLevel::Metadata,
        CaptureAccess::Url => ContentLevel::Url,
        CaptureAccess::Full => ContentLevel::Full,
    }
}

pub fn source_for_access(access: CaptureAccess, preferred: SnapshotSource) -> SnapshotSource {
    if access == CaptureAccess::Metadata {
        SnapshotSource::Workspace
    } else {
        preferred
    }
}

fn transition_reason(previous: &Snapshot, current: &Snapshot) -> TransitionReason {
    if previous.app.app_id != current.app.app_id {
        return TransitionReason::AppChanged;
    }

    if previous.focused_window_id != current.focused_window_id {
        return TransitionReason::WindowChanged;
    }

    if previous.activity_kind != current.activity_kind {
        return TransitionReason::ActivityKindChanged;
    }

    if previous.url != current.url {
        return TransitionReason::UrlChanged;
    }

    if previous.window_title != current.window_title {
        return TransitionReason::TitleChanged;
    }

    if previous.text_anchor_kind != current.text_anchor_kind
        || previous.text_anchor_identity != current.text_anchor_identity
    {
        return TransitionReason::TextAnchorChanged;
    }

    TransitionReason::ContentChanged
}

fn watch_loop<F>(
    mut poll_snapshot: F,
    options: WatchOptions,
    stop: Arc<StopSignal>,
    sample_tx: tokio::sync::mpsc::UnboundedSender<Result<RawCaptureSample, CaptureError>>,
) where
    F: FnMut() -> Result<Option<NormalizedSnapshot>, CaptureError>,
{
    let mut first_iteration = true;

    loop {
        if !first_iteration && stop.wait_timeout(options.poll_interval) {
            break;
        }
        let should_emit = !first_iteration || options.emit_initial;
        first_iteration = false;

        if stop.is_set() {
            break;
        }

        match poll_snapshot() {
            Ok(snapshot) => {
                if !should_emit {
                    continue;
                }

                if sample_tx
                    .send(Ok(RawCaptureSample {
                        captured_at: snapshot
                            .as_ref()
                            .map(|value| value.captured_at)
                            .unwrap_or_else(std::time::SystemTime::now),
                        snapshot,
                    }))
                    .is_err()
                {
                    break;
                }
            }
            Err(error) => {
                let _ = sample_tx.send(Err(error));
                break;
            }
        }
    }
}

#[derive(Default)]
struct StopSignal {
    stopped: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl StopSignal {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.condvar.notify_all();
    }

    fn is_set(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn wait_timeout(&self, duration: Duration) -> bool {
        if self.is_set() {
            return true;
        }

        let guard = self.mutex.lock().unwrap_or_else(|error| error.into_inner());
        let result = self
            .condvar
            .wait_timeout_while(guard, duration, |_| !self.is_set());
        match result {
            Ok((_, _)) => self.is_set(),
            Err(error) => {
                let _ = error.into_inner();
                self.is_set()
            }
        }
    }
}

struct WatchStream {
    inner: tokio_stream::wrappers::UnboundedReceiverStream<Result<RawCaptureSample, CaptureError>>,
    stop: Arc<StopSignal>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Stream for WatchStream {
    type Item = Result<RawCaptureSample, CaptureError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for WatchStream {
    fn drop(&mut self) {
        self.stop.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_identity() -> AppIdentity {
        AppIdentity {
            pid: 42,
            app_name: "TextEdit".to_string(),
            app_id: "com.apple.TextEdit".to_string(),
            app_id_kind: AppIdKind::BundleId,
            bundle_id: Some("com.apple.TextEdit".to_string()),
            executable_path: None,
        }
    }

    fn snapshot(title: &str) -> NormalizedSnapshot {
        NormalizedSnapshot {
            app: app_identity(),
            activity_kind: ActivityKind::ForegroundWindow,
            captured_at: std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            pid: 42,
            app_name: "TextEdit".to_string(),
            bundle_id: Some("com.apple.TextEdit".to_string()),
            focused_window_id: Some(101),
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
    fn content_fingerprint_is_stable() {
        assert_eq!(
            snapshot("Notes").content_fingerprint(),
            snapshot("Notes").content_fingerprint()
        );
    }

    #[test]
    fn content_fingerprint_prefers_anchor_text_over_ambient_text() {
        let mut left = snapshot("Notes");
        let mut right = snapshot("Notes");
        left.visible_text = Some("ambient one".to_string());
        right.visible_text = Some("ambient two".to_string());

        assert_eq!(left.content_fingerprint(), right.content_fingerprint());
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
    fn browser_policy_sanitizes_url() {
        let policy = BrowserPolicy::default();
        let sanitized = policy
            .sanitize_url("https://docs.example.com/page?q=1#section")
            .unwrap();

        assert_eq!(sanitized.host.as_deref(), Some("docs.example.com"));
        assert_eq!(sanitized.url, "https://docs.example.com/page");
    }
}
