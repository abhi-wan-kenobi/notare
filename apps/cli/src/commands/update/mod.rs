use hypr_cli_tui::run_screen;

mod app;
mod screen;
mod ui;

use self::app::App;
use self::screen::UpdateScreen;

pub enum UpdateOutcome {
    RunUpdate,
    Continue,
}

pub async fn run(
    current: String,
    latest: String,
    action: &crate::update_check::UpdateAction,
) -> UpdateOutcome {
    let screen = UpdateScreen::new(App::new(current, latest, action.command_str()));

    run_screen::<UpdateScreen>(screen, None)
        .await
        .unwrap_or(UpdateOutcome::Continue)
}
