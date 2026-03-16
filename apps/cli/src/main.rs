mod cli;
mod commands;
mod error;
mod output;
mod runtime;
mod theme;
mod widgets;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Commands};
use crate::error::CliResult;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.global.no_color || std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(cli.verbose.tracing_level_filter().into())
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .init();

    if let Err(error) = run(cli).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> CliResult<()> {
    let Cli {
        command,
        global,
        verbose,
    } = cli;

    match command {
        Commands::Chat { session, prompt } => {
            commands::chat::run(commands::chat::Args {
                session,
                prompt,
                api_key: global.api_key,
                model: global.model,
            })
            .await
        }
        Commands::Connect { r#type, provider } => {
            commands::connect::run(commands::connect::Args {
                connection_type: r#type,
                provider,
                base_url: global.base_url,
                api_key: global.api_key,
            })?;
            eprintln!("Next: run `char status` to verify");
            Ok(())
        }
        Commands::Status => commands::status::run(),
        Commands::Auth => {
            commands::auth::run()?;
            eprintln!("Opened auth page in browser");
            eprintln!("Next: run `char connect` to configure a provider");
            Ok(())
        }
        Commands::Desktop => {
            use commands::desktop::DesktopAction;
            match commands::desktop::run()? {
                DesktopAction::OpenedApp => eprintln!("Opened desktop app"),
                DesktopAction::OpenedDownloadPage => {
                    eprintln!("Desktop app not found — opened download page")
                }
            }
            Ok(())
        }
        Commands::Listen { provider, audio } => {
            commands::listen::run(commands::listen::Args {
                stt: commands::SttGlobalArgs {
                    provider,
                    base_url: global.base_url,
                    api_key: global.api_key,
                    model: global.model,
                    language: global.language,
                },
                record: global.record,
                audio,
            })
            .await
        }
        Commands::Batch { args } => {
            let stt = commands::SttGlobalArgs {
                provider: args.provider,
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
                language: global.language,
            };
            commands::batch::run(args, stt, verbose.is_silent()).await
        }
        Commands::Model { command } => commands::model::run(command).await,
        #[cfg(debug_assertions)]
        Commands::Debug { command } => commands::debug::run(command).await,
        Commands::Completions { shell } => {
            cli::generate_completions(shell);
            Ok(())
        }
    }
}
