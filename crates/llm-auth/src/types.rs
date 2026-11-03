use std::collections::HashMap;

/// Supported LLM OAuth providers.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Anthropic,
    OpenaiCodex,
    Gemini,
}

impl ProviderId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenaiCodex => "openai_codex",
            Self::Gemini => "gemini",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "anthropic" => Some(Self::Anthropic),
            "openai_codex" => Some(Self::OpenaiCodex),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }
}

/// Authentication status for a provider.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AuthStatus {
    pub signed_in: bool,
    pub account_label: Option<String>,
    pub expires_at: Option<String>,
}

/// OAuth completion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    Loopback,
    Paste,
}

/// Outcome of starting an OAuth login flow.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct StartFlowResult {
    pub authorize_url: String,
    pub mode: AuthMode,
}

/// Access token result returned to callers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AccessTokenResult {
    pub token: String,
    pub extra: HashMap<String, String>,
}

/// Dynamic catalog of supported LLM auth providers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CatalogResult {
    pub providers: Vec<CatalogProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CatalogProvider {
    pub id: ProviderId,
    pub enabled: bool,
    pub label: String,
    pub beta_headers: HashMap<String, String>,
    pub system_prefix: Option<String>,
    pub base_url: String,
    pub notes: String,
    /// Models offered in the subscription picker. Absent in catalogs
    /// published before this field existed, hence the default.
    #[serde(default)]
    pub models: Vec<CatalogModel>,
    #[serde(default)]
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CatalogModel {
    pub id: String,
    pub label: String,
}
