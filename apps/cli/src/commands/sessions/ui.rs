use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::KeyHints;

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;
    let area = centered_dialog(frame.area());

    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" Sessions ")
        .border_style(theme.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [content_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    if app.loading() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  Loading...", theme.muted))),
            content_area,
        );
    } else if let Some(error) = app.error() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!("  {error}"), theme.error))),
            content_area,
        );
    } else if app.sessions().is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  No sessions found", theme.muted))),
            content_area,
        );
    } else {
        draw_list(frame, app, content_area, &theme);
    }

    let hints = vec![("↑/↓", "navigate"), ("Enter", "select"), ("Esc", "back")];
    frame.render_widget(KeyHints::new(&theme).hints(hints), status_area);
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let items: Vec<ListItem> = app
        .sessions()
        .iter()
        .map(|s| {
            let title = s.title.as_deref().unwrap_or("Untitled").to_string();
            let date = s.created_at.clone();
            let short_date = date.get(..10).unwrap_or(&date).to_string();
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(short_date, theme.muted),
                Span::raw("  "),
                Span::styled(title, Style::new().add_modifier(Modifier::BOLD)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("  > ");

    frame.render_stateful_widget(list, area, app.list_state_mut());
}

fn centered_dialog(area: Rect) -> Rect {
    let width = area.width.saturating_mul(3).saturating_div(5).clamp(40, 80);
    let height = area
        .height
        .saturating_mul(3)
        .saturating_div(5)
        .clamp(12, 30);
    let [v] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [h] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(v);
    h
}
