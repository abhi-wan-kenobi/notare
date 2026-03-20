mod action;
mod app;
mod audio;
mod effect;
mod runtime;
mod screen;
mod server;
mod ui;

use hypr_cli_tui::run_screen;
use tokio::sync::mpsc;

pub use super::{DebugProvider, TranscribeArgs as DebugTranscribeArgs, TranscribeMode};
use crate::error::{CliError, CliResult};

use self::app::App;
use self::runtime::Runtime;
use self::screen::TranscribeScreen;

pub async fn run(args: DebugTranscribeArgs) -> CliResult<()> {
    let mode = args.mode.clone();
    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::start(args, tx).await?;
    let screen = TranscribeScreen::new(App::new(mode, crate::tui_trace::capture()));

    let result = run_screen(screen, Some(rx))
        .await
        .map_err(|e| CliError::operation_failed("run transcribe screen", e.to_string()));
    runtime.abort();
    result
}
