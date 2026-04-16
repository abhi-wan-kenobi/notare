use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a notification from Codex
    Notify {
        /// JSON payload from Codex
        payload: String,
    },
    /// Install char as the Codex notify handler
    Install,
    /// Remove char from the Codex notify handler
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
    let event: hypr_codex::NotifyEvent = serde_json::from_str(payload)
        .map_err(|e| CliError::invalid_argument("payload", payload.to_string(), e.to_string()))?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let response = hypr_agent_core::install_cli(hypr_agent_core::InstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Codex,
    })
    .map_err(|e| CliError::operation_failed("install codex integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let response = hypr_agent_core::uninstall_cli(hypr_agent_core::UninstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Codex,
    })
    .map_err(|e| CliError::operation_failed("uninstall codex integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}
