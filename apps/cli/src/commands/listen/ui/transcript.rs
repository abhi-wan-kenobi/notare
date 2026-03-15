use hypr_transcript::{Segment, SegmentKey, SegmentWord, SpeakerLabeler};
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use crate::commands::listen::app::App;
use crate::fmt::format_timestamp_ms;
use crate::theme::Theme;

const FADE_IN_SECS: f64 = 0.4;

pub(super) fn draw_transcript(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    elapsed: std::time::Duration,
    theme: &Theme,
) {
    let segments = app.segments();

    let border_style = if app.transcript_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    let inner_area = block.inner(area);

    if segments.is_empty() {
        let empty_message = if app.can_accept_audio_drop() {
            "Drop an audio file to transcribe..."
        } else {
            "Waiting for speech..."
        };
        let lines = vec![Line::from(Span::styled(empty_message, theme.placeholder))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        app.update_transcript_max_scroll(0);
        return;
    }

    app.check_new_segments(segments.len(), inner_area);

    // border (1) + padding (1) on each side = 4 chars
    let content_width = area.width.saturating_sub(4) as usize;
    let lines = build_segment_lines(&segments, theme, content_width, app);

    let line_count = lines.len();
    let paragraph = Paragraph::new(lines).block(block);

    let visible_lines = area.height.saturating_sub(2) as usize;
    let max_scroll = line_count
        .saturating_sub(visible_lines)
        .min(u16::MAX as usize) as u16;
    app.update_transcript_max_scroll(max_scroll);

    let paragraph = paragraph.scroll((app.scroll_offset(), 0));
    frame.render_widget(paragraph, area);

    app.process_effects(elapsed, frame.buffer_mut(), inner_area);

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

fn build_segment_lines<'a>(
    segments: &[Segment],
    theme: &Theme,
    content_width: usize,
    app: &App,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let mut labeler = SpeakerLabeler::from_segments(segments, None);

    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }

        // Header: speaker label + timestamp
        let label = speaker_label(&segment.key, &mut labeler);
        let timestamp = segment
            .words
            .first()
            .map(|w| format_timestamp_ms(w.start_ms))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(label, theme.speaker_label),
            Span::raw(" "),
            Span::styled(format!("[{timestamp}]"), theme.timestamp),
        ]));

        // Build word spans, wrapping manually to respect content width.
        // We join words into flowing text and wrap at word boundaries.
        let indent = "  ";
        let wrap_width = content_width.saturating_sub(indent.len());

        if wrap_width == 0 {
            continue;
        }

        let mut current_spans: Vec<Span> = vec![Span::raw(indent.to_string())];
        let mut current_len = 0usize;

        for (j, word) in segment.words.iter().enumerate() {
            let text = &word.text;
            let separator = if j > 0 { " " } else { "" };
            let needed = separator.len() + text.len();

            // Wrap to next line if adding this word would exceed width
            if current_len > 0 && current_len + needed > wrap_width {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
                current_spans = vec![Span::raw(indent.to_string())];
                current_len = 0;
            } else if !separator.is_empty() {
                current_spans.push(Span::raw(separator.to_string()));
                current_len += separator.len();
            }

            let style = word_style(word, theme, app);
            current_spans.push(Span::styled(text.clone(), style));
            current_len += text.len();
        }

        if current_len > 0 {
            lines.push(Line::from(current_spans));
        }
    }

    lines
}

fn word_style(word: &SegmentWord, theme: &Theme, app: &App) -> Style {
    if !word.is_final {
        return theme.transcript_partial;
    }

    if let Some(ref id) = word.id {
        let age = app.word_age_secs(id);
        if age < FADE_IN_SECS {
            return fade_in_style(age);
        }
    }

    theme.transcript_final
}

fn fade_in_style(age: f64) -> Style {
    let t = (age / FADE_IN_SECS).clamp(0.0, 1.0);
    let t = ease_out_cubic(t);
    let start = 50u8;
    let end = 220u8;
    let v = start + ((end - start) as f64 * t) as u8;
    Style::new().fg(Color::Rgb(v, v, v))
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn speaker_label(key: &SegmentKey, labeler: &mut SpeakerLabeler) -> String {
    labeler.label_for(key, None)
}
