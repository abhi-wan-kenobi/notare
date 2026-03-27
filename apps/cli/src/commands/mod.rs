pub mod transcribe;

#[cfg(feature = "desktop")]
pub mod export;
#[cfg(feature = "desktop")]
pub mod humans;
#[cfg(feature = "task")]
pub mod integration;
#[cfg(feature = "desktop")]
pub mod meetings;
#[cfg(feature = "desktop")]
pub mod orgs;

#[cfg(feature = "standalone")]
pub mod bug;
#[cfg(feature = "standalone")]
pub mod desktop;
#[cfg(feature = "standalone")]
pub mod hello;
#[cfg(feature = "standalone")]
pub mod model;
#[cfg(feature = "standalone")]
pub mod record;

use crate::app::AppContext;
use crate::cli::{Cli, Commands as CliCommand};
use crate::error::CliResult;

pub async fn run(ctx: &AppContext, command: Option<CliCommand>) -> CliResult<()> {
    match command {
        Some(CliCommand::Transcribe { args }) => transcribe::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Models { args }) => model::run(ctx, args).await,
        #[cfg(feature = "standalone")]
        Some(CliCommand::Record { args }) => record::run(ctx, args).await,
        Some(CliCommand::Completions { shell }) => {
            crate::cli::generate_completions(shell);
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Desktop) => {
            use desktop::DesktopAction;
            match desktop::run()? {
                DesktopAction::OpenedApp => eprintln!("Opened desktop app"),
                DesktopAction::OpenedDownloadPage => {
                    eprintln!("Desktop app not found — opened download page")
                }
            }
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Bug) => {
            bug::run()?;
            eprintln!("Opened bug report page in browser");
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(CliCommand::Hello) => {
            hello::run()?;
            eprintln!("Opened char.com in browser");
            Ok(())
        }
        #[cfg(feature = "task")]
        Some(CliCommand::Claude { command }) => integration::claude::run(command).await,
        #[cfg(feature = "task")]
        Some(CliCommand::Codex { command }) => integration::codex::run(command).await,
        #[cfg(feature = "task")]
        Some(CliCommand::Opencode { command }) => integration::opencode::run(command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Meetings { command }) => meetings::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Humans { command }) => humans::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Orgs { command }) => orgs::run(ctx, command).await,
        #[cfg(feature = "desktop")]
        Some(CliCommand::Export { command }) => export::run(ctx, command).await,
        None => {
            use clap::CommandFactory;

            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}
