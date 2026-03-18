mod context_panel;
mod header;
mod input;
mod status_bar;
mod transcript;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::theme::Theme;
use crate::widgets::AppShell;

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let [main_area, status_area] = AppShell::new(&theme).render(frame);

    let [chat_col, context_col] =
        Layout::horizontal([Constraint::Min(40), Constraint::Length(28)]).areas(main_area);

    let [header_area, body_area, input_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .areas(chat_col);

    header::draw(frame, app, header_area, &theme);
    transcript::draw(frame, app, body_area, &theme);
    input::draw(frame, app, input_area, &theme);
    context_panel::draw(frame, app, context_col, &theme);
    status_bar::draw(frame, app, status_area, &theme);
}
