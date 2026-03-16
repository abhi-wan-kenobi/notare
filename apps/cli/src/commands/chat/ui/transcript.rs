use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders};

use crate::commands::chat::app::{App, Speaker};
use crate::theme::Theme;
use crate::widgets::{Transcript, TranscriptEntry};

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = Theme::default();

    let mut entries: Vec<TranscriptEntry> = app
        .transcript()
        .iter()
        .map(|m| {
            let (label, style) = match m.speaker {
                Speaker::User => ("You", Style::new()),
                Speaker::Assistant => ("Assistant", theme.transcript_final),
                Speaker::Error => ("Error", theme.error),
            };
            TranscriptEntry {
                label: label.to_string(),
                content: m.content.clone(),
                label_style: theme.speaker_label,
                content_style: style,
            }
        })
        .collect();

    if app.streaming() || !app.pending_assistant().is_empty() {
        entries.push(TranscriptEntry {
            label: "Assistant".to_string(),
            content: app.pending_assistant().to_string(),
            label_style: theme.speaker_label,
            content_style: theme.transcript_final,
        });
    }

    let transcript = Transcript::new(entries)
        .placeholder(Span::styled(
            "No messages yet. Start typing below.",
            theme.placeholder,
        ))
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(" Transcript "),
        );

    frame.render_stateful_widget(transcript, area, app.scroll_state_mut());
}
