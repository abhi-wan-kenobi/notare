use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};
use textwrap::wrap;

use crate::commands::chat::app::{App, Speaker};
use crate::theme::Theme;
use crate::widgets::Scrollable;

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = Theme::default();

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Transcript ");
    let inner = block.inner(area);
    let width = inner.width.saturating_sub(2) as usize;
    let lines = build_lines(app, width, &theme);

    let scrollable = Scrollable::new(lines).block(block);
    let scroll_state = app.scroll_state_mut();
    frame.render_stateful_widget(scrollable, area, scroll_state);
}

fn build_lines(app: &App, width: usize, theme: &Theme) -> Vec<Line<'static>> {
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
