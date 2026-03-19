pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
pub(crate) mod ui;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

enum ExternalEvent {
    Loaded(Vec<hypr_db_app::OrganizationRow>),
    LoadError(String),
}

struct OrgsScreen {
    app: App,
}

impl OrgsScreen {
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

impl Screen for OrgsScreen {
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
            ExternalEvent::Loaded(orgs) => Action::Loaded(orgs),
            ExternalEvent::LoadError(msg) => Action::LoadError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("orgs"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}

pub async fn run(pool: SqlitePool) -> CliResult<Option<String>> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        match hypr_db_app::list_organizations(&pool).await {
            Ok(orgs) => {
                let _ = external_tx.send(ExternalEvent::Loaded(orgs));
            }
            Err(e) => {
                let _ = external_tx.send(ExternalEvent::LoadError(e.to_string()));
            }
        }
    });

    let screen = OrgsScreen { app: App::new() };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("orgs tui", e.to_string()))
}

pub async fn add(pool: &SqlitePool, name: &str) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_organization(pool, &id, name)
        .await
        .map_err(|e| CliError::operation_failed("insert organization", e.to_string()))?;
    println!("{id}");
    Ok(())
}

pub async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    match hypr_db_app::get_organization(pool, id).await {
        Ok(Some(org)) => {
            println!("id: {}", org.id);
            println!("name: {}", org.name);
            println!("created_at: {}", org.created_at);
            Ok(())
        }
        Ok(None) => Err(CliError::msg(format!("organization '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

pub async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_organization(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete organization", e.to_string()))?;
    eprintln!("deleted {id}");
    Ok(())
}
