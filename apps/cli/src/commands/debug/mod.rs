mod transcribe;

use clap::Subcommand;

use crate::error::CliResult;
use transcribe::TranscribeArgs;

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
        DebugCommands::Transcribe { args } => transcribe::run(args).await,
    }
}
