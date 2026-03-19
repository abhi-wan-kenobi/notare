use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;

use crate::widgets::{PermissionButton, PermissionStatus};

use super::super::app::App;
use super::super::runtime::CalendarPermissionState;

pub(crate) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let [label_area, button_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);

    frame.render_widget(
        Paragraph::new("  Calendar access is required to list your calendars."),
        label_area,
    );

    let status = match app.cal_auth_status() {
        None => PermissionStatus::Checking,
        Some(CalendarPermissionState::NotDetermined) => PermissionStatus::NotRequested,
        Some(CalendarPermissionState::Authorized) => PermissionStatus::Authorized,
        Some(CalendarPermissionState::Denied) => PermissionStatus::Denied,
    };

    frame.render_widget(PermissionButton::new(status), button_area);
}
