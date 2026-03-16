use std::sync::Arc;
use std::time::Instant;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};
use owhisper_interface::stream::StreamResponse;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::audio::{ChannelKind, DisplayMode};
use super::shell::TranscribeShell;
use crate::widgets::TracingCapture;

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}

pub(super) enum RawEvent {
    StreamResponse {
        response: StreamResponse,
        display_mode: DisplayMode,
    },
    StreamEnded,
}

struct ChannelTranscript {
    segments: Vec<String>,
    partial: String,
    t0: Instant,
    kind: ChannelKind,
    last_confirmed: Option<String>,
}

impl ChannelTranscript {
    fn new(t0: Instant, kind: ChannelKind) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
            kind,
            last_confirmed: None,
        }
    }

    fn elapsed(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn set_partial(&mut self, text: &str) {
        self.partial = text.to_string();
    }

    fn confirm(&mut self, text: &str) {
        if self.last_confirmed.as_deref() == Some(text) {
            return;
        }
        self.last_confirmed = Some(text.to_string());
        self.segments.push(text.to_string());
        self.partial.clear();
    }

    fn render_line(&self) -> Option<Line<'static>> {
        let confirmed: String = self
            .segments
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if confirmed.is_empty() && self.partial.is_empty() {
            return None;
        }

        let to = self.elapsed();
        let from_str = if self.segments.is_empty() {
            "--:--".to_string()
        } else {
            fmt_ts(0.0)
        };

        let prefix = format!("[{} / {}]", from_str, fmt_ts(to));

        let (confirmed_style, partial_style) = match self.kind {
            ChannelKind::Mic => (
                Style::new()
                    .fg(Color::Rgb(255, 190, 190))
                    .add_modifier(Modifier::BOLD),
                Style::new().fg(Color::Rgb(128, 95, 95)),
            ),
            ChannelKind::Speaker => (
                Style::new()
                    .fg(Color::Rgb(190, 200, 255))
                    .add_modifier(Modifier::BOLD),
                Style::new().fg(Color::Rgb(95, 100, 128)),
            ),
        };

        let mut spans = vec![
            Span::styled(prefix, Style::new().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(confirmed, confirmed_style),
        ];

        if !self.partial.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(self.partial.clone(), partial_style));
        }

        Some(Line::from(spans))
    }
}

pub(super) struct RawTranscribeScreen {
    shell: TranscribeShell,
    channels: Vec<ChannelTranscript>,
}

impl RawTranscribeScreen {
    pub(super) fn new(tracing: Arc<TracingCapture>) -> Self {
        Self {
            shell: TranscribeShell::new(tracing),
            channels: Vec::new(),
        }
    }

    fn ensure_channels(&mut self, mode: &DisplayMode) {
        if !self.channels.is_empty() {
            return;
        }
        let t0 = Instant::now();
        match mode {
            DisplayMode::Single(kind) => {
                self.channels.push(ChannelTranscript::new(t0, *kind));
            }
            DisplayMode::Dual => {
                self.channels
                    .push(ChannelTranscript::new(t0, ChannelKind::Mic));
                self.channels
                    .push(ChannelTranscript::new(t0, ChannelKind::Speaker));
            }
        }
    }

    fn transcript_lines(&self) -> Vec<Line<'static>> {
        self.channels
            .iter()
            .filter_map(|ch| ch.render_line())
            .collect()
    }
}

impl Screen for RawTranscribeScreen {
    type ExternalEvent = RawEvent;
    type Output = ();

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        if let TuiEvent::Key(key) = event {
            if let Some(ctrl) = self.shell.handle_key(key) {
                return ctrl;
            }
        }
        ScreenControl::Continue
    }

    fn on_external_event(
        &mut self,
        event: RawEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            RawEvent::StreamResponse {
                response,
                display_mode,
            } => {
                self.ensure_channels(&display_mode);

                if let StreamResponse::TranscriptResponse {
                    is_final,
                    channel,
                    channel_index,
                    ..
                } = response
                {
                    let text = channel
                        .alternatives
                        .first()
                        .map(|a| a.transcript.as_str())
                        .unwrap_or("");

                    let ch = match &display_mode {
                        DisplayMode::Single(_) => 0,
                        DisplayMode::Dual => {
                            channel_index.first().copied().unwrap_or(0).clamp(0, 1) as usize
                        }
                    };

                    if ch < self.channels.len() {
                        if is_final {
                            self.channels[ch].confirm(text);
                        } else {
                            self.channels[ch].set_partial(text);
                        }
                    }
                }
            }
            RawEvent::StreamEnded => {
                self.shell.stream_ended = true;
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut Frame) {
        let lines = self.transcript_lines();
        self.shell.draw(
            frame,
            "Transcript",
            lines,
            "Stream ended.",
            Style::new().fg(Color::Cyan),
        );
    }

    fn title(&self) -> String {
        "debug transcribe (raw)".into()
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(50)
    }
}
