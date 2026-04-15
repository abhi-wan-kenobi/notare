use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

const STOP_EVENT: &str = "Stop";
const COMMAND: &str = "char claude notify";

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = hypr_claude::health_check_with_options(&hypr_claude::ClaudeOptions {
        claude_path_override: options.claude_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Claude,
        binary_path: health.binary_path,
        installed: health.installed,
        integration_installed: integration_installed().unwrap_or(false),
        version: health.version,
        status: health.status.into(),
        auth_status: health.auth_status.into(),
        message: health.message,
    }
}

pub fn install_cli() -> Result<InstallCliResponse, String> {
    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::upsert_command_hook(&mut settings, STOP_EVENT, COMMAND)?;
    hypr_claude::write_settings(&settings_path, &settings)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Claude,
        target_path: settings_path.clone(),
        message: format!(
            "Installed char as Claude Code hook handler in {}",
            settings_path.display()
        ),
    })
}

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
    let settings_path = hypr_claude::settings_path();
    let mut settings = hypr_claude::read_settings(&settings_path)?;

    hypr_claude::remove_command_hook(&mut settings, STOP_EVENT, COMMAND)?;
    hypr_claude::write_settings(&settings_path, &settings)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Claude,
        target_path: settings_path.clone(),
        message: format!(
            "Removed char as Claude Code hook handler from {}",
            settings_path.display()
        ),
    })
}

fn integration_installed() -> Result<bool, String> {
    let settings_path = hypr_claude::settings_path();
    let settings = hypr_claude::read_settings(&settings_path)?;
    Ok(hypr_claude::has_command_hook(
        &settings, STOP_EVENT, COMMAND,
    ))
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

impl From<hypr_claude::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_claude::HealthAuthStatus) -> Self {
        match value {
            hypr_claude::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_claude::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_claude::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}
