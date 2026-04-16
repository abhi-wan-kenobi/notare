use clap::Subcommand;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// Receive a hook event from Claude Code (reads JSON from stdin)
    Notify,
    /// Install char as a Claude Code hook handler
    Install,
    /// Remove char from Claude Code hooks
    Uninstall,
}

pub async fn run(command: Commands) -> CliResult<()> {
    match command {
        Commands::Notify => notify(),
        Commands::Install => install(),
        Commands::Uninstall => uninstall(),
    }
}

fn notify() -> CliResult<()> {
    let event = super::read_stdin_json()?;

    // TODO: write to app DB
    super::print_pretty_json(&event)
}

fn install() -> CliResult<()> {
    let response = hypr_agent_core::install_cli(hypr_agent_core::InstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Claude,
    })
    .map_err(|e| CliError::operation_failed("install claude integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}

fn uninstall() -> CliResult<()> {
    let response = hypr_agent_core::uninstall_cli(hypr_agent_core::UninstallCliRequest {
        provider: hypr_agent_core::ProviderKind::Claude,
    })
    .map_err(|e| CliError::operation_failed("uninstall claude integration", e))?;

    eprintln!("{}", response.message);
    Ok(())
}
