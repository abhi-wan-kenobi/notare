use hypr_cli_tui::run_screen;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

mod action;
mod app;
mod effect;
mod screen;
mod ui;

pub enum EntryCommand {
    MeetingsNew,
    Chat { session_id: Option<String> },
    View { session_id: String },
}

pub enum EntryAction {
    Launch(EntryCommand),
    Model(crate::commands::model::Commands),
    Quit,
}

use self::app::App;
use self::screen::{EntryScreen, ExternalEvent};

pub struct Args {
    pub status_message: Option<String>,
    pub initial_command: Option<String>,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub pool: SqlitePool,
}

pub async fn run(args: Args) -> EntryAction {
    let (external_tx, external_rx) = mpsc::unbounded_channel();
    let (connect_tx, mut connect_rx) = mpsc::unbounded_channel();

    {
        let external_tx = external_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = connect_rx.recv().await {
                if external_tx
                    .send(ExternalEvent::ConnectRuntime(event))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    let pool = args.pool.clone();
    let mut screen = EntryScreen::new(
        App::new(args.status_message, args.stt_provider, args.llm_provider),
        external_tx,
        crate::commands::connect::runtime::Runtime::new(connect_tx),
        pool,
    );

    if let Some(command) = args.initial_command {
        if let Some(action) = screen.submit_initial_command(command) {
            return action;
        }
    }

    run_screen::<EntryScreen>(screen, Some(external_rx))
        .await
        .unwrap_or(EntryAction::Quit)
}
