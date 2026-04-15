use crate::{
    HealthCheckOptions, InstallCliResponse, ProviderAuthStatus, ProviderHealth,
    ProviderHealthStatus, ProviderKind, UninstallCliResponse,
};

pub fn health(options: &HealthCheckOptions) -> ProviderHealth {
    let health = hypr_opencode::health_check_with_options(&hypr_opencode::OpencodeOptions {
        opencode_path_override: options.opencode_path_override.clone(),
        ..Default::default()
    });

    ProviderHealth {
        provider: ProviderKind::Opencode,
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
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists() && !hypr_opencode::has_char_plugin(&plugin_path)? {
        return Err(format!(
            "refusing to replace existing plugin at {}",
            plugin_path.display()
        ));
    }

    hypr_opencode::write_plugin(&plugin_path)?;

    Ok(InstallCliResponse {
        provider: ProviderKind::Opencode,
        target_path: plugin_path.clone(),
        message: format!(
            "Installed char as OpenCode plugin at {}",
            plugin_path.display()
        ),
    })
}

pub fn uninstall_cli() -> Result<UninstallCliResponse, String> {
    let plugin_path = hypr_opencode::plugin_path();

    if plugin_path.exists() && !hypr_opencode::has_char_plugin(&plugin_path)? {
        return Err(format!(
            "refusing to remove existing plugin at {}",
            plugin_path.display()
        ));
    }

    hypr_opencode::remove_plugin(&plugin_path)?;

    Ok(UninstallCliResponse {
        provider: ProviderKind::Opencode,
        target_path: plugin_path.clone(),
        message: format!(
            "Removed char as OpenCode plugin from {}",
            plugin_path.display()
        ),
    })
}

fn integration_installed() -> Result<bool, String> {
    let plugin_path = hypr_opencode::plugin_path();
    hypr_opencode::has_char_plugin(&plugin_path)
}

impl From<hypr_opencode::HealthStatus> for ProviderHealthStatus {
    fn from(value: hypr_opencode::HealthStatus) -> Self {
        match value {
            hypr_opencode::HealthStatus::Ready => Self::Ready,
            hypr_opencode::HealthStatus::Warning => Self::Warning,
            hypr_opencode::HealthStatus::Error => Self::Error,
        }
    }
}

impl From<hypr_opencode::HealthAuthStatus> for ProviderAuthStatus {
    fn from(value: hypr_opencode::HealthAuthStatus) -> Self {
        match value {
            hypr_opencode::HealthAuthStatus::Authenticated => Self::Authenticated,
            hypr_opencode::HealthAuthStatus::Unauthenticated => Self::Unauthenticated,
            hypr_opencode::HealthAuthStatus::Unknown => Self::Unknown,
        }
    }
}
