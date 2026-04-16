use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a hook event from OpenCode
    Notify {
        /// JSON payload from OpenCode
        payload: String,
    },
    /// Install char as an OpenCode plugin
    Install,
    /// Remove char from OpenCode plugins
    Uninstall,
}

pub async fn run(command: Commands) -> CliResult<()> {
    match command {
        Commands::Notify { payload } => notify(&payload),
        Commands::Install => install(),
        Commands::Uninstall => uninstall(),
    }
}

fn notify(payload: &str) -> CliResult<()> {
    let event: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| CliError::invalid_argument("payload", payload.to_string(), e.to_string()))?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let response = hypr_agent_core::install_cli(hypr_agent_core::InstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Opencode,
    })
    .map_err(|e| CliError::operation_failed("install opencode integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let response = hypr_agent_core::uninstall_cli(hypr_agent_core::UninstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Opencode,
    })
    .map_err(|e| CliError::operation_failed("uninstall opencode integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}
