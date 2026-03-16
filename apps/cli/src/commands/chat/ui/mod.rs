mod header;
mod input;
mod status_bar;
mod transcript;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let [header_area, body_area, input_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    header::draw(frame, app, header_area);
    transcript::draw(frame, app, body_area);
    input::draw(frame, app, input_area);
    status_bar::draw(frame, app, status_area);
}
