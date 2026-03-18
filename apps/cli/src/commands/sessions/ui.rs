use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints, SelectList};

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let inner = CenteredDialog::new("Sessions", &theme).render(frame);

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

    frame.render_stateful_widget(SelectList::new(items, theme), area, app.list_state_mut());
}
