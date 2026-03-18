use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct CommandBar<'a> {
    buffer: &'a str,
}

impl<'a> CommandBar<'a> {
    pub fn new(buffer: &'a str) -> Self {
        Self { buffer }
    }
}

impl Widget for CommandBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let line = Line::from(vec![
            Span::styled(" COMMAND ", Style::new().fg(Color::Black).bg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(format!(":{}", self.buffer), Style::new().fg(Color::White)),
            Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
        ]);
        Paragraph::new(line).render(area, buf);
    }
}
