use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear};

use crate::theme::Theme;

pub struct CenteredDialog<'a> {
    title: &'a str,
    theme: &'a Theme,
    width_frac: (u16, u16),
    height_frac: (u16, u16),
    width_range: (u16, u16),
    height_range: (u16, u16),
    pad_x: u16,
    pad_y: u16,
}

impl<'a> CenteredDialog<'a> {
    pub fn new(title: &'a str, theme: &'a Theme) -> Self {
        Self {
            title,
            theme,
            width_frac: (2, 5),
            height_frac: (3, 5),
            width_range: (40, 60),
            height_range: (12, 30),
            pad_x: 3,
            pad_y: 2,
        }
    }

    pub fn wide(mut self) -> Self {
        self.width_frac = (4, 5);
        self.height_frac = (4, 5);
        self.width_range = (60, 140);
        self.height_range = (16, 50);
        self.pad_x = 2;
        self.pad_y = 1;
        self
    }

    pub fn render(&self, frame: &mut Frame) -> Rect {
        frame.render_widget(
            Block::default().style(Style::new().bg(self.theme.dialog.overlay_bg)),
            frame.area(),
        );

        let area = self.centered_area(frame.area());

        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default().style(Style::new().bg(self.theme.dialog.bg)),
            area,
        );

        let padded = Rect {
            x: area.x + self.pad_x,
            y: area.y + self.pad_y,
            width: area.width.saturating_sub(self.pad_x * 2),
            height: area.height.saturating_sub(self.pad_y * 2),
        };

        let [title_area, _gap, content_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(padded);

        let [title_left, title_right] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(3)]).areas(title_area);

        frame.render_widget(
            Span::styled(
                self.title,
                Style::new()
                    .fg(self.theme.dialog.title_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            title_left,
        );

        frame.render_widget(Span::styled("esc", self.theme.muted), title_right);

        content_area
    }

    fn centered_area(&self, area: Rect) -> Rect {
        let width = area
            .width
            .saturating_mul(self.width_frac.0)
            .saturating_div(self.width_frac.1)
            .clamp(self.width_range.0, self.width_range.1);
        let height = area
            .height
            .saturating_mul(self.height_frac.0)
            .saturating_div(self.height_frac.1)
            .clamp(self.height_range.0, self.height_range.1);
        let [v] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(area);
        let [h] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(v);
        h
    }
}
