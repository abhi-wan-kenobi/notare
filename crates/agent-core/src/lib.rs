use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub value: Option<String>,
}

pub fn ping(payload: PingRequest) -> PingResponse {
    PingResponse {
        value: payload.value,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum ProviderHealthStatus {
    Ready,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthStatus {
    Authenticated,
    Unauthenticated,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub provider: ProviderKind,
    pub binary_path: PathBuf,
    pub installed: bool,
    pub version: Option<String>,
    pub status: ProviderHealthStatus,
    pub auth_status: ProviderAuthStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HealthCheckOptions {
    pub codex_path_override: Option<PathBuf>,
    pub claude_path_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResponse {
    pub providers: Vec<ProviderHealth>,
}

pub fn health_check() -> HealthCheckResponse {
    health_check_with_options(&HealthCheckOptions::default())
}

pub fn health_check_with_options(options: &HealthCheckOptions) -> HealthCheckResponse {
    let codex = hypr_codex::health_check_with_options(&hypr_codex::CodexOptions {
        codex_path_override: options.codex_path_override.clone(),
        ..Default::default()
    });
    let claude = hypr_claude::health_check_with_options(&hypr_claude::ClaudeOptions {
        claude_path_override: options.claude_path_override.clone(),
        ..Default::default()
    });

    HealthCheckResponse {
        providers: vec![codex.into(), claude.into()],
    }
}

impl From<hypr_codex::HealthCheck> for ProviderHealth {
    fn from(value: hypr_codex::HealthCheck) -> Self {
        Self {
            provider: ProviderKind::Codex,
            binary_path: value.binary_path,
            installed: value.installed,
            version: value.version,
            status: value.status.into(),
            auth_status: value.auth_status.into(),
            message: value.message,
        }
    }
}

impl From<hypr_claude::HealthCheck> for ProviderHealth {
    fn from(value: hypr_claude::HealthCheck) -> Self {
        Self {
            provider: ProviderKind::Claude,
            binary_path: value.binary_path,
            installed: value.installed,
            version: value.version,
            status: value.status.into(),
            auth_status: value.auth_status.into(),
            message: value.message,
        }
    }
}

impl From<hypr_codex::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_codex::HealthStatus) -> Self {
        match value {
            hypr_codex::HealthStatus::Ready => Self::Ready,
            hypr_codex::HealthStatus::Warning => Self::Warning,
            hypr_codex::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_claude::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_claude::HealthStatus) -> Self {
        match value {
            hypr_claude::HealthStatus::Ready => Self::Ready,
            hypr_claude::HealthStatus::Warning => Self::Warning,
            hypr_claude::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_codex::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_codex::HealthAuthStatus) -> Self {
        match value {
            hypr_codex::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_codex::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_codex::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<hypr_claude::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_claude::HealthAuthStatus) -> Self {
        match value {
            hypr_claude::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_claude::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_claude::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}
