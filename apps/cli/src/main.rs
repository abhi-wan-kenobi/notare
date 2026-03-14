mod commands;
mod error;
mod event;
mod frame;
mod terminal;
mod textarea_input;
mod theme;

use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};

use crate::commands::batch;
use crate::commands::debug::DebugCommands;
use crate::commands::model::ModelCommands;
use crate::error::CliResult;

/// Live transcription and audio tools
#[derive(Parser)]
#[command(
    name = "char",
    version,
    propagate_version = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    global: GlobalArgs,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,
}

#[derive(clap::Args)]
struct GlobalArgs {
    #[arg(long, global = true, env = "CHAR_BASE_URL", value_parser = parse_base_url)]
    base_url: Option<String>,

    #[arg(long, global = true, env = "CHAR_API_KEY")]
    api_key: Option<String>,

    #[arg(long, global = true, env = "CHAR_MODEL")]
    model: Option<String>,

    #[arg(long, global = true, env = "CHAR_LANGUAGE", default_value = "en")]
    language: String,

    #[arg(long, global = true, env = "CHAR_RECORD")]
    record: bool,
}

fn parse_base_url(value: &str) -> Result<String, String> {
    let parsed = url::Url::parse(value).map_err(|e| format!("invalid URL '{value}': {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!(
            "invalid URL '{value}': scheme must be http or https"
        ));
    }
    Ok(value.to_string())
}

#[derive(Subcommand)]
enum Commands {
    /// Start live transcription (TUI)
    Listen,
    /// Authenticate with char.com
    Auth,
    /// Open the desktop app or download page
    Desktop,
    /// Transcribe an audio file
    Batch {
        #[command(flatten)]
        args: batch::BatchArgs,
    },
    /// Manage local models
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },
    /// Debug and diagnostic tools
    Debug {
        #[command(subcommand)]
        command: DebugCommands,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli).await {
        eprintln!("{:?}", miette::Report::new(error));
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
        Commands::Auth => {
            commands::auth::run()?;
            eprintln!("Opened auth page in browser");
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
        Commands::Listen => {
            commands::listen::run(commands::listen::Args {
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
                language: global.language,
                record: global.record,
            })
            .await
        }
        Commands::Batch { args } => {
            commands::batch::run(
                args,
                global.base_url,
                global.api_key,
                global.model,
                global.language,
                verbose.is_silent(),
            )
            .await
        }
        Commands::Model { command } => commands::model::run(command).await,
        Commands::Debug { command } => commands::debug::run(command).await,
    }
}
