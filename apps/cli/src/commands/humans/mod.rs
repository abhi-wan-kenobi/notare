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
    Loaded(Vec<hypr_db_app::HumanRow>),
    LoadError(String),
}

struct HumansScreen {
    app: App,
}

impl HumansScreen {
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

impl Screen for HumansScreen {
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
            ExternalEvent::Loaded(humans) => Action::Loaded(humans),
            ExternalEvent::LoadError(msg) => Action::LoadError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("humans"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}

pub async fn run(pool: SqlitePool) -> CliResult<Option<String>> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        match hypr_db_app::list_humans(&pool).await {
            Ok(humans) => {
                let _ = external_tx.send(ExternalEvent::Loaded(humans));
            }
            Err(e) => {
                let _ = external_tx.send(ExternalEvent::LoadError(e.to_string()));
            }
        }
    });

    let screen = HumansScreen { app: App::new() };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("humans tui", e.to_string()))
}

pub async fn add(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    org: Option<&str>,
    title: Option<&str>,
) -> CliResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    hypr_db_app::insert_human(
        pool,
        &id,
        name,
        email.unwrap_or(""),
        org.unwrap_or(""),
        title.unwrap_or(""),
    )
    .await
    .map_err(|e| CliError::operation_failed("insert human", e.to_string()))?;
    println!("{id}");
    Ok(())
}

pub async fn show(pool: &SqlitePool, id: &str) -> CliResult<()> {
    match hypr_db_app::get_human(pool, id).await {
        Ok(Some(h)) => {
            println!("id: {}", h.id);
            println!("name: {}", h.name);
            println!("email: {}", h.email);
            println!("org_id: {}", h.org_id);
            println!("job_title: {}", h.job_title);
            println!("created_at: {}", h.created_at);
            Ok(())
        }
        Ok(None) => Err(CliError::msg(format!("human '{id}' not found"))),
        Err(e) => Err(CliError::operation_failed("query", e.to_string())),
    }
}

pub async fn rm(pool: &SqlitePool, id: &str) -> CliResult<()> {
    hypr_db_app::delete_human(pool, id)
        .await
        .map_err(|e| CliError::operation_failed("delete human", e.to_string()))?;
    eprintln!("deleted {id}");
    Ok(())
}
