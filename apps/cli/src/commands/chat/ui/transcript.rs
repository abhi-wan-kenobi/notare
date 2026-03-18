use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use textwrap::wrap;

use crate::commands::chat::app::{App, Speaker};
use crate::theme::Theme;
use crate::widgets::render_scrollable;

// --- Data layer: describe what to render ---

enum Entry<'a> {
    Message { speaker: Speaker, content: &'a str },
    Pending { content: &'a str },
    Placeholder,
}

fn entries(app: &App) -> Vec<Entry<'_>> {
    let transcript = app.transcript();

    if transcript.is_empty() && !app.streaming() && app.pending_assistant().is_empty() {
        return vec![Entry::Placeholder];
    }

    let mut out: Vec<Entry<'_>> = transcript
        .iter()
        .map(|msg| Entry::Message {
            speaker: msg.speaker,
            content: &msg.content,
        })
        .collect();

    if app.streaming() || !app.pending_assistant().is_empty() {
        out.push(Entry::Pending {
            content: app.pending_assistant(),
        });
    }

    out
}

// --- View layer: how to render each entry ---

fn render_entry(entry: &Entry<'_>, wrap_width: usize, theme: &Theme) -> Vec<Line<'static>> {
    match entry {
        Entry::Message { speaker, content } => render_message(*speaker, content, wrap_width, theme),
        Entry::Pending { content } => render_pending(content, wrap_width, theme),
        Entry::Placeholder => {
            vec![Line::from(Span::styled(
                "    Start a conversation below.",
                theme.placeholder,
            ))]
        }
    }
}

fn render_message(
    speaker: Speaker,
    content: &str,
    wrap_width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let wrapped = wrap(content, wrap_width);

    match speaker {
        Speaker::User => wrapped
            .iter()
            .map(|w| {
                Line::from(vec![
                    Span::styled("  \u{258e} ", theme.user_bar),
                    Span::raw(w.to_string()),
                ])
            })
            .collect(),
        Speaker::Assistant => wrapped
            .iter()
            .map(|w| Line::from(Span::styled(format!("    {w}"), theme.transcript_final)))
            .collect(),
        Speaker::Error => wrapped
            .iter()
            .map(|w| Line::from(Span::styled(format!("    {w}"), theme.error)))
            .collect(),
    }
}

fn render_pending(content: &str, wrap_width: usize, theme: &Theme) -> Vec<Line<'static>> {
    if content.is_empty() {
        vec![Line::from(Span::styled(
            "    ...",
            theme.transcript_partial,
        ))]
    } else {
        wrap(content, wrap_width)
            .iter()
            .map(|w| Line::from(Span::styled(format!("    {w}"), theme.transcript_final)))
            .collect()
    }
}

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let width = area.width as usize;
    let wrap_width = width.saturating_sub(6).max(8);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for entry in &entries(app) {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(render_entry(entry, wrap_width, theme));
    }

    render_scrollable(frame, lines, None, area, app.scroll_state_mut());
}
