use crossterm::terminal::SetTitle;
use hypr_listener_core::State;
use hypr_transcript::{Segment, SegmentKey, SegmentWord, SpeakerLabeler};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use super::app::{App, Mode};
use super::waveform::build_waveform_spans;
use crate::theme::Theme;

const FADE_IN_SECS: f64 = 0.4;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let elapsed = app.frame_elapsed();
    let theme = Theme::default();
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header_bar(frame, app, header_area, &theme);

    let [memo_area, transcript_area] = Layout::horizontal([
        Constraint::Percentage(app.notepad_width_percent()),
        Constraint::Percentage(100 - app.notepad_width_percent()),
    ])
    .areas(body_area);

    draw_notepad(frame, app, memo_area, &theme);
    draw_transcript(frame, app, transcript_area, elapsed, &theme);
    draw_status_bar(frame, app, status_area, &theme);
    update_terminal_title(app);
}

fn update_terminal_title(app: &App) {
    let time = super::format_hhmmss(app.elapsed());
    let title = format!("char: {} ({time})", app.status);
    let _ = crossterm::execute!(std::io::stdout(), SetTitle(title));
}

fn draw_header_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let time_str = super::format_hhmmss(app.elapsed());

    let state_style = match app.state {
        State::Active if app.degraded.is_some() => theme.status_degraded,
        State::Active => theme.status_active,
        State::Finalizing => theme.status_degraded,
        State::Inactive => theme.status_inactive,
    };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(&app.status, state_style),
        Span::styled("  |  ", theme.muted),
        Span::raw(time_str),
        Span::styled("  |  ", theme.muted),
        Span::raw(format!("{} words", app.words.len())),
    ];

    if let Some(err) = app.errors.last() {
        spans.push(Span::styled("  |  ", theme.muted));
        spans.push(Span::styled(err, theme.error));
    }

    if app.mic_muted {
        spans.push(Span::styled("  |  ", theme.muted));
        spans.push(Span::styled("mic muted", theme.muted));
    }

    // Waveform on the right side
    let waveform_width = 20usize;
    spans.push(Span::raw("  "));
    spans.extend(build_waveform_spans(app, waveform_width, theme));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_transcript(
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

    let paragraph = paragraph.scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, area);

    app.process_effects(elapsed, frame.buffer_mut(), inner_area);

    let mut scrollbar_state =
        ScrollbarState::new(line_count.max(1)).position(app.scroll_offset as usize);
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

use crate::fmt::format_timestamp_ms;

fn draw_notepad(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let border_style = if app.memo_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Notepad ");

    app.set_memo_block(block);
    frame.render_widget(app.memo(), area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mode = app.mode();

    let line = match mode {
        Mode::Command => {
            let cmd_display = format!(":{}", app.command_buffer);
            Line::from(vec![
                Span::styled(" COMMAND ", Style::new().fg(Color::Black).bg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(cmd_display, Style::new().fg(Color::White)),
                Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
            ])
        }
        Mode::Insert => Line::from(vec![
            Span::styled(" INSERT ", Style::new().fg(Color::Black).bg(Color::Green)),
            Span::raw(" "),
            Span::styled("[esc]", theme.shortcut_key),
            Span::raw(" normal  "),
            Span::styled("[tab]", theme.shortcut_key),
            Span::raw(" normal  "),
            Span::styled("[ctrl+z/y]", theme.shortcut_key),
            Span::raw(" undo/redo  "),
            Span::styled("[ctrl+u]", theme.shortcut_key),
            Span::raw(" clear"),
        ]),
        Mode::Normal => Line::from(vec![
            Span::styled(" NORMAL ", Style::new().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(" "),
            Span::styled("[:q]", theme.shortcut_key),
            Span::raw(" quit  "),
            Span::styled("[j/k]", theme.shortcut_key),
            Span::raw(" scroll  "),
            Span::styled("[i]", theme.shortcut_key),
            Span::raw(" notepad  "),
            Span::styled("[G/g]", theme.shortcut_key),
            Span::raw(" bottom/top  "),
            Span::styled(format!("{} words", app.words.len()), theme.muted),
        ]),
    };

    frame.render_widget(Paragraph::new(line), area);
}
