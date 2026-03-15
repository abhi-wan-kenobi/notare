use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use textwrap::wrap;

use super::app::{App, Speaker};
use crate::fmt::format_hhmmss;
use crate::theme::Theme;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();
    let [header_area, body_area, input_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, app, header_area, &theme);
    draw_transcript(frame, app, body_area, &theme);
    draw_input(frame, app, input_area, &theme);
    draw_status(frame, app, status_area, &theme);
}

fn draw_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, theme: &Theme) {
    let mut spans = vec![
        Span::raw(" "),
        Span::styled("chat", theme.status_active),
        Span::styled("  |  ", theme.muted),
        Span::raw(app.model().to_string()),
    ];

    if let Some(session) = app.session() {
        spans.push(Span::styled("  |  ", theme.muted));
        spans.push(Span::raw(format!("session {session}")));
    }

    spans.push(Span::styled("  |  ", theme.muted));
    spans.push(Span::raw(format_hhmmss(app.elapsed())));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_transcript(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect, theme: &Theme) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Transcript ");
    let inner = block.inner(area);
    let width = inner.width.saturating_sub(2) as usize;
    let lines = build_transcript_lines(app, width, theme);
    let visible_lines = inner.height as usize;
    let line_count = lines.len();
    let max_scroll = line_count
        .saturating_sub(visible_lines)
        .min(u16::MAX as usize) as u16;
    app.update_max_scroll(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.scroll_offset(), 0));
    frame.render_widget(paragraph, area);

    let mut scrollbar_state =
        ScrollbarState::new(line_count.max(1)).position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

fn build_transcript_lines(app: &App, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let wrap_width = width.max(8);

    for message in app.transcript() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(render_message(
            message.speaker,
            &message.content,
            wrap_width,
            theme,
        ));
    }

    if app.streaming() || !app.pending_assistant().is_empty() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(render_message(
            Speaker::Assistant,
            app.pending_assistant(),
            wrap_width,
            theme,
        ));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No messages yet. Start typing below.",
            theme.placeholder,
        )));
    }

    lines
}

fn draw_input(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect, theme: &Theme) {
    let title = if app.status().starts_with("Streaming") {
        " Composer (locked) "
    } else {
        " Composer "
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border_focused)
        .title(title);
    app.input_mut().set_block(block);
    frame.render_widget(app.input(), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, theme: &Theme) {
    let status_style = if app.last_error().is_some() {
        theme.error
    } else if app.status().starts_with("Streaming") {
        theme.status_active
    } else {
        theme.muted
    };

    let status = Line::from(vec![
        Span::raw(" "),
        Span::styled(app.status().to_string(), status_style),
        Span::styled("  |  ", theme.muted),
        Span::raw("Enter submit"),
        Span::styled("  |  ", theme.muted),
        Span::raw("Ctrl+C quit"),
        Span::styled("  |  ", theme.muted),
        Span::raw("Ctrl+Up/Down or PgUp/PgDn scroll"),
    ]);
    frame.render_widget(Paragraph::new(status), area);
}

fn render_message(
    speaker: Speaker,
    content: &str,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let (label, style) = match speaker {
        Speaker::User => ("You", Style::new()),
        Speaker::Assistant => ("Assistant", theme.transcript_final),
        Speaker::Error => ("Error", theme.error),
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{label}: "), theme.speaker_label),
        Span::styled(String::new(), style),
    ])];

    let wrapped = wrap(content, width.saturating_sub(2).max(8));
    if wrapped.is_empty() {
        lines.push(Line::from(Span::styled("  ", style)));
    } else {
        lines.extend(
            wrapped
                .into_iter()
                .map(|line| Line::from(Span::styled(format!("  {line}"), style))),
        );
    }

    lines
}
