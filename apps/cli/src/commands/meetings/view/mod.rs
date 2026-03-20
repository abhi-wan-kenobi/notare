mod action;
mod app;
mod effect;
mod runtime;
mod screen;
mod ui;

use hypr_cli_tui::run_screen;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::app::App;
use self::runtime::Runtime;
use self::screen::ViewScreen;

pub struct Args {
    pub meeting_id: String,
    pub pool: SqlitePool,
}

pub async fn run(args: Args) -> CliResult<()> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    let runtime = Runtime::new(args.pool, external_tx);
    runtime.load(args.meeting_id.clone());

    let screen = ViewScreen::new(App::new(args.meeting_id), runtime);

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("view tui", e.to_string()))
}
