use clap::Subcommand;

use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Claude Code integration
    Claude {
        #[command(subcommand)]
        command: crate::commands::integration::claude::Commands,
    },
    /// Codex integration
    Codex {
        #[command(subcommand)]
        command: crate::commands::integration::codex::Commands,
    },
    /// OpenCode integration
    Opencode {
        #[command(subcommand)]
        command: crate::commands::integration::opencode::Commands,
    },
}

pub async fn run(command: Option<Commands>) -> CliResult<()> {
    match command {
        Some(Commands::Claude { command }) => {
            crate::commands::integration::claude::run(command).await
        }
        Some(Commands::Codex { command }) => {
            crate::commands::integration::codex::run(command).await
        }
        Some(Commands::Opencode { command }) => {
            crate::commands::integration::opencode::run(command).await
        }
        None => {
            eprintln!("Todo is not ready yet.");
            Ok(())
        }
    }
}
