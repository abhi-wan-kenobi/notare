#[cfg(feature = "_cli-audio")]
pub mod transcribe;
#[cfg(feature = "_cli-audio")]
pub(crate) mod update_check;

#[cfg(feature = "todo")]
pub mod integration;
#[cfg(feature = "todo")]
pub mod todo;

#[cfg(feature = "_cli-desktop")]
pub mod bug;
#[cfg(feature = "_cli-desktop")]
pub mod desktop;
#[cfg(feature = "_cli-desktop")]
pub mod hello;
#[cfg(feature = "_cli-audio")]
pub mod model;
#[cfg(feature = "_cli-audio")]
pub mod play;
#[cfg(feature = "_cli-audio")]
pub mod record;
#[cfg(feature = "_cli-skill-update")]
pub mod skill;
#[cfg(feature = "_cli-skill-update")]
pub mod update;

use std::path::{Path, PathBuf};

use crate::app::AppContext;
use crate::cli::{Cli, Commands as CliCommand};
use crate::error::{CliError, CliResult};

pub(crate) fn resolve_session_dir(base: Option<&Path>, timestamp: &str) -> CliResult<PathBuf> {
    let base = base.map(Path::to_path_buf).unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("char")
    });

    let mut dir = base.join(timestamp);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| CliError::operation_failed("create session directory", e.to_string()))?;
        return Ok(dir);
    }

    for i in 1.. {
        dir = base.join(format!("{timestamp}-{i}"));
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                CliError::operation_failed("create session directory", e.to_string())
            })?;
            return Ok(dir);
        }
    }

    unreachable!()
}

pub async fn run(ctx: &AppContext, command: Option<CliCommand>) -> CliResult<()> {
    #[cfg(not(any(feature = "_cli-audio", feature = "_cli-skill-update")))]
    let _ = ctx;

    match command {
        #[cfg(feature = "_cli-audio")]
        Some(CliCommand::Transcribe { args }) => transcribe::run(ctx, args).await,
        #[cfg(feature = "_cli-audio")]
        Some(CliCommand::Models { args }) => model::run(ctx, args).await,
        #[cfg(feature = "_cli-audio")]
        Some(CliCommand::Play { args }) => play::run(ctx, args).await,
        #[cfg(feature = "_cli-audio")]
        Some(CliCommand::Record { args }) => record::run(ctx, args).await,
        #[cfg(feature = "_cli-skill-update")]
        Some(CliCommand::Skill { command }) => skill::run(ctx, command).await,
        Some(CliCommand::Completions { shell }) => {
            crate::cli::generate_completions(shell);
            Ok(())
        }
        #[cfg(feature = "_cli-desktop")]
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
        #[cfg(feature = "_cli-db")]
        Some(CliCommand::Db { args }) => hypr_db_cli::run(args)
            .await
            .map_err(|e| e.to_string().into()),
        #[cfg(feature = "_cli-desktop")]
        Some(CliCommand::Bug) => {
            bug::run()?;
            eprintln!("Opened bug report page in browser");
            Ok(())
        }
        #[cfg(feature = "_cli-desktop")]
        Some(CliCommand::Hello) => {
            hello::run()?;
            eprintln!("Opened char.com in browser");
            Ok(())
        }
        #[cfg(feature = "_cli-skill-update")]
        Some(CliCommand::Update) => update::run(),
        #[cfg(feature = "todo")]
        Some(CliCommand::Todo { command }) => todo::run(command).await,
        None => {
            use clap::CommandFactory;

            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}
