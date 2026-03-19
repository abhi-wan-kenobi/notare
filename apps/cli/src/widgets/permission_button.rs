use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Checking,
    NotRequested,
    Authorized,
    Denied,
}

pub struct PermissionButton {
    status: PermissionStatus,
}

impl PermissionButton {
    pub fn new(status: PermissionStatus) -> Self {
        Self { status }
    }
}

impl Widget for PermissionButton {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            return;
        }

        let (status_text, status_color) = match self.status {
            PermissionStatus::Checking => ("Checking...", Color::DarkGray),
            PermissionStatus::NotRequested => ("Not Requested", Color::Yellow),
            PermissionStatus::Authorized => ("Authorized", Color::Green),
            PermissionStatus::Denied => ("Denied", Color::Red),
        };

        let status_line = Line::from(vec![
            Span::raw("  Status: "),
            Span::styled(status_text, Style::new().fg(status_color)),
        ]);
        status_line.render(area, buf);

        let hint = match self.status {
            PermissionStatus::Checking => "",
            PermissionStatus::NotRequested => "  [Enter] Request Access",
            PermissionStatus::Authorized => "  [Enter] Continue",
            PermissionStatus::Denied => "  [Enter] Reset in System Settings",
        };

        if !hint.is_empty() && area.height >= 2 {
            let hint_area = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            Line::from(Span::styled(hint, Style::new().fg(Color::DarkGray))).render(hint_area, buf);
        }
    }
}
