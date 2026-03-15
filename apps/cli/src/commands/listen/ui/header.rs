use hypr_listener_core::State;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::waveform::build_waveform_spans;
use crate::commands::listen::app::App;
use crate::theme::Theme;

const WAVEFORM_WIDTH: usize = 20;
use crate::fmt::format_hhmmss;

pub(super) fn draw_header_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let time_str = format_hhmmss(app.elapsed());

    let state_style = match app.listener_state() {
        State::Active if app.degraded().is_some() => theme.status_degraded,
        State::Active => theme.status_active,
        State::Finalizing => theme.status_degraded,
        State::Inactive => theme.status_inactive,
    };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(app.status(), state_style),
        Span::styled("  |  ", theme.muted),
        Span::raw(time_str),
        Span::styled("  |  ", theme.muted),
        Span::raw(format!("{} words", app.word_count())),
    ];

    if let Some(err) = app.last_error() {
        spans.push(Span::styled("  |  ", theme.muted));
        spans.push(Span::styled(err, theme.error));
    }

    if app.mic_muted() {
        spans.push(Span::styled("  |  ", theme.muted));
        spans.push(Span::styled("mic muted", theme.muted));
    }

    spans.push(Span::raw("  "));
    spans.extend(build_waveform_spans(app, WAVEFORM_WIDTH, theme));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
