use clap::Subcommand;

use super::transcribe::TranscribeArgs;
use crate::error::CliResult;

#[derive(Subcommand)]
pub enum DebugCommands {
    /// Real-time transcription from audio devices
    Transcribe {
        #[command(flatten)]
        args: TranscribeArgs,
    },
}

pub async fn run(command: DebugCommands) -> CliResult<()> {
    match command {
        DebugCommands::Transcribe { args } => super::transcribe::run(args).await,
    }
}
