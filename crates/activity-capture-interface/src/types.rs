use std::time::SystemTime;

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CaptureAccess {
    None,
    #[default]
    Metadata,
    Url,
    Full,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    #[default]
    OptIn,
    OptOut,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BundleRule {
    pub bundle_id: String,
    pub access: CaptureAccess,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DomainRule {
    pub domain: String,
    pub include_subdomains: bool,
    pub access: CaptureAccess,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BrowserPolicy {
    pub rules: Vec<DomainRule>,
    pub require_url_for_text_access: bool,
    pub block_private_browsing: bool,
    pub strip_query: bool,
    pub strip_fragment: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CapturePolicy {
    pub mode: PolicyMode,
    pub app_rules: Vec<BundleRule>,
    pub browser: BrowserPolicy,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AppIdKind {
    BundleId,
    ExecutablePath,
    ProcessName,
    Pid,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ContentLevel {
    Metadata,
    Url,
    Full,
}

impl ContentLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Url => "url",
            Self::Full => "full",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    Accessibility,
    Workspace,
}

impl SnapshotSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    ForegroundWindow,
    Browser,
    AudioSession,
}

impl ActivityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ForegroundWindow => "foreground_window",
            Self::Browser => "browser",
            Self::AudioSession => "audio_session",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TextAnchorKind {
    FocusedEdit,
    SelectedText,
    FocusedElement,
    Document,
    None,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TextAnchorConfidence {
    High,
    Medium,
    Low,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AppIdentity {
    pub pid: i32,
    pub app_name: String,
    pub app_id: String,
    pub app_id_kind: AppIdKind,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct BrowserContext {
    pub raw_url: Option<String>,
    pub is_private: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CaptureCandidate {
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub source: SnapshotSource,
    pub browser: Option<BrowserContext>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SanitizedBrowserUrl {
    pub url: String,
    pub host: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CaptureDecision {
    pub access: CaptureAccess,
    pub skip: bool,
    pub source: SnapshotSource,
    pub activity_kind: ActivityKind,
    pub url: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TextAnchor {
    pub kind: TextAnchorKind,
    pub identity: String,
    pub text: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub selected_text: Option<String>,
    pub confidence: TextAnchorConfidence,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SnapshotSpec {
    pub captured_at: SystemTime,
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub access: CaptureAccess,
    pub source: SnapshotSource,
    pub focused_window_id: Option<u32>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub text_anchor: Option<TextAnchor>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Snapshot {
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub captured_at: SystemTime,
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

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Event {
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub fingerprint: String,
    pub snapshot: Snapshot,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Transition {
    pub previous: Option<Event>,
    pub current: Option<Event>,
    pub reason: TransitionReason,
    pub sequence: u64,
    pub suppressed_snapshot_count: u32,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Capabilities {
    pub can_watch: bool,
    pub can_capture_visible_text: bool,
    pub can_capture_browser_url: bool,
    pub requires_accessibility_permission: bool,
}
