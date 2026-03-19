use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::Theme;
use crate::widgets::{MultiSelect, MultiSelectEntry, MultiSelectState};

use super::super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if app.cal_loading() {
        frame.render_widget(
            Span::styled("  Loading calendars...", Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    if let Some(err) = app.cal_error() {
        frame.render_widget(Span::styled(format!("  Error: {err}"), theme.error), area);
        return;
    }

    if app.cal_items().is_empty() {
        frame.render_widget(
            Span::styled("  No calendars found", Style::new().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let items = app.cal_items();
    let mut entries: Vec<MultiSelectEntry> = Vec::new();
    let mut current_source = "";

    for (i, item) in items.iter().enumerate() {
        if item.source.as_str() != current_source {
            if !current_source.is_empty() {
                entries.push(MultiSelectEntry::Group(Line::from("")));
            }
            entries.push(MultiSelectEntry::Group(Line::from(Span::styled(
                item.source.clone(),
                Style::new().fg(Color::DarkGray),
            ))));
            current_source = &item.source;
        }

        let checked = app.cal_enabled().get(i).copied().unwrap_or(false);
        let color_dot = parse_hex_color(&item.color);
        let label = Line::from(vec![
            Span::styled("\u{25CF} ", Style::new().fg(color_dot)),
            Span::raw(item.name.clone()),
        ]);
        entries.push(MultiSelectEntry::Item { checked, label });
    }

    let data_idx = app.cal_list_state_mut().selected().unwrap_or(0);
    let mut state = MultiSelectState::new(data_idx);

    frame.render_stateful_widget(MultiSelect::new(entries, theme), area, &mut state);
}

fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::White
}
