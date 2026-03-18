pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
pub(crate) mod ui;
pub(crate) mod view;

use std::path::PathBuf;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
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

pub async fn run(db_path: PathBuf) -> CliResult<Option<String>> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        match load_sessions(db_path).await {
            Ok(sessions) => {
                let _ = external_tx.send(ExternalEvent::Loaded(sessions));
            }
            Err(e) => {
                let _ = external_tx.send(ExternalEvent::LoadError(e));
            }
        }
    });

    let screen = SessionsScreen { app: App::new() };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("sessions tui", e.to_string()))
}

pub(crate) async fn load_sessions(
    db_path: PathBuf,
) -> Result<Vec<hypr_db_app::SessionRow>, String> {
    let db = hypr_db_core2::Db3::connect_local_plain(&db_path)
        .await
        .map_err(|e| format!("failed to open database: {e}"))?;

    hypr_db_app::migrate(db.pool())
        .await
        .map_err(|e| format!("migration failed: {e}"))?;

    hypr_db_app::list_sessions(db.pool())
        .await
        .map_err(|e| format!("query failed: {e}"))
}
