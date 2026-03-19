pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
pub(crate) mod ui;
pub(crate) mod view;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

enum ExternalEvent {
    Loaded(Vec<hypr_db_app::SessionRow>),
    LoadError(String),
}

struct SessionsScreen {
    app: App,
}

impl SessionsScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<String>> {
        for effect in effects {
            match effect {
                Effect::Select(id) => return ScreenControl::Exit(Some(id)),
                Effect::Exit => return ScreenControl::Exit(None),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for SessionsScreen {
    type ExternalEvent = ExternalEvent;
    type Output = Option<String>;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::Loaded(sessions) => Action::Loaded(sessions),
            ExternalEvent::LoadError(msg) => Action::LoadError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("sessions"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}

pub async fn run(pool: SqlitePool) -> CliResult<Option<String>> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        match hypr_db_app::list_sessions(&pool).await {
            Ok(sessions) => {
                let _ = external_tx.send(ExternalEvent::Loaded(sessions));
            }
            Err(e) => {
                let _ = external_tx.send(ExternalEvent::LoadError(e.to_string()));
            }
        }
    });

    let screen = SessionsScreen { app: App::new() };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("sessions tui", e.to_string()))
}

pub(crate) async fn load_sessions(
    pool: SqlitePool,
) -> Result<Vec<hypr_db_app::SessionRow>, String> {
    hypr_db_app::list_sessions(&pool)
        .await
        .map_err(|e| format!("query failed: {e}"))
}

pub async fn participants(pool: &SqlitePool, session_id: &str) -> CliResult<()> {
    let rows = hypr_db_app::list_session_participants(pool, session_id)
        .await
        .map_err(|e| CliError::operation_failed("query", e.to_string()))?;

    for row in &rows {
        println!("{}\t{}", row.human_id, row.source);
    }
    Ok(())
}

pub async fn add_participant(pool: &SqlitePool, session_id: &str, human_id: &str) -> CliResult<()> {
    hypr_db_app::add_session_participant(pool, session_id, human_id, "manual")
        .await
        .map_err(|e| CliError::operation_failed("add participant", e.to_string()))?;
    eprintln!("added {human_id} to {session_id}");
    Ok(())
}

pub async fn remove_participant(
    pool: &SqlitePool,
    session_id: &str,
    human_id: &str,
) -> CliResult<()> {
    hypr_db_app::remove_session_participant(pool, session_id, human_id)
        .await
        .map_err(|e| CliError::operation_failed("remove participant", e.to_string()))?;
    eprintln!("removed {human_id} from {session_id}");
    Ok(())
}
