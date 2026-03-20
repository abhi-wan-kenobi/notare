use clap::ValueEnum;
use hypr_cli_tui::run_screen;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::CliResult;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConfigureTab {
    Stt,
    Llm,
    Calendar,
}

mod action;
mod app;
mod effect;
mod runtime;
mod screen;
mod ui;

use self::app::App;
use self::runtime::Runtime;
use self::screen::ConfigureScreen;

pub async fn run(pool: &SqlitePool, cli_tab: Option<ConfigureTab>) -> CliResult<()> {
    let initial_tab = cli_tab.map(|t| match t {
        ConfigureTab::Stt => app::Tab::Stt,
        ConfigureTab::Llm => app::Tab::Llm,
        ConfigureTab::Calendar => app::Tab::Calendar,
    });

    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::new(pool.clone(), tx);

    let (app, initial_effects) = App::new(initial_tab);
    let mut screen = ConfigureScreen::new(app, runtime);

    screen.apply_effects(initial_effects);

    run_screen(screen, Some(rx)).await.map_err(|e| {
        crate::error::CliError::operation_failed("run configure screen", e.to_string())
    })
}
