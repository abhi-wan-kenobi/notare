use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let install_label = format!("Update now (npm install -g char@{})", app.npm_tag);
    let items: [&str; 3] = [&install_label, "Skip", "Don't remind for this version"];

    let area = frame.area();
    let content_height = 2 + items.len() as u16;

    let [content_area] = Layout::vertical([Constraint::Length(content_height)])
        .flex(Flex::Center)
        .areas(area);

    let [centered] = Layout::horizontal([Constraint::Length(55)])
        .flex(Flex::Center)
        .areas(content_area);

    let constraints: Vec<Constraint> = std::iter::once(Constraint::Length(1))
        .chain(std::iter::once(Constraint::Length(1)))
        .chain(items.iter().map(|_| Constraint::Length(1)))
        .collect();
    let rows = Layout::vertical(constraints).split(centered);

    let title = Line::from(vec![
        Span::raw("Update available: "),
        Span::styled(
            format!("v{}", app.current),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" → "),
        Span::styled(
            format!("v{}", app.latest),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), rows[0]);

    for (i, label) in items.iter().enumerate() {
        let row_idx = i + 2;
        let (prefix, style) = if i == app.selected {
            (
                "> ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::DarkGray))
        };
        let line = Line::from(Span::styled(format!("{prefix}{label}"), style));
        frame.render_widget(Paragraph::new(line), rows[row_idx]);
    }
}
