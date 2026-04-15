use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = hypr_codex::health_check_with_options(&hypr_codex::CodexOptions {
        codex_path_override: options.codex_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Codex,
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
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();

    let mut table = hypr_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !hypr_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to replace existing notify handler in {}",
            config_path.display()
        ));
    }

    hypr_codex::set_notify(&mut table, command);
    hypr_codex::write_config(&config_path, &table)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Installed char as Codex notify handler in {}",
            config_path.display()
        ),
    })
}

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
    let config_path = hypr_codex::config_path();
    let command = hypr_codex::notify_command();
    let mut table = hypr_codex::read_config(&config_path)?;

    if table.contains_key("notify") && !hypr_codex::has_notify(&table, &command) {
        return Err(format!(
            "refusing to remove existing notify handler in {}",
            config_path.display()
        ));
    }

    hypr_codex::remove_notify(&mut table);
    hypr_codex::write_config(&config_path, &table)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Codex,
        target_path: config_path.clone(),
        message: format!(
            "Removed char as Codex notify handler from {}",
            config_path.display()
        ),
    })
}

fn integration_installed() -> Result<bool, String> {
    let config_path = hypr_codex::config_path();
    let table = hypr_codex::read_config(&config_path)?;
    Ok(hypr_codex::has_notify(
        &table,
        &hypr_codex::notify_command(),
    ))
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

impl From<hypr_codex::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_codex::HealthAuthStatus) -> Self {
        match value {
            hypr_codex::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_codex::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_codex::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}
