use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent};
use hypr_transcript::{
    FinalizedWord, PartialWord, RuntimeSpeakerHint, Segment, SegmentBuilderOptions,
    TranscriptDelta, TranscriptProcessor, WordRef,
};
use owhisper_interface::stream::StreamResponse;
use ratatui::Frame;

use super::shell::TranscribeShell;
use crate::theme::Theme;
use crate::widgets::{TracingCapture, build_segment_lines};

pub(super) enum RichEvent {
    StreamResponse(StreamResponse),
    StreamEnded,
}

pub(super) struct RichTranscribeScreen {
    shell: TranscribeShell,
    words: Vec<FinalizedWord>,
    partials: Vec<PartialWord>,
    hints: Vec<RuntimeSpeakerHint>,
    partial_hints: Vec<RuntimeSpeakerHint>,
    transcript: TranscriptProcessor,
    word_first_seen: HashMap<String, Instant>,
    theme: Theme,
}

impl RichTranscribeScreen {
    pub(super) fn new(tracing: Arc<TracingCapture>) -> Self {
        Self {
            shell: TranscribeShell::new(tracing),
            words: Vec::new(),
            partials: Vec::new(),
            hints: Vec::new(),
            partial_hints: Vec::new(),
            transcript: TranscriptProcessor::new(),
            word_first_seen: HashMap::new(),
            theme: Theme::default(),
        }
    }

    fn segments(&self) -> Vec<Segment> {
        let opts = SegmentBuilderOptions {
            max_gap_ms: Some(5000),
            ..Default::default()
        };
        let mut all_hints = self.hints.clone();
        let final_words_count = self.words.len();
        all_hints.extend(self.partial_hints.iter().cloned().map(|mut hint| {
            if let WordRef::RuntimeIndex(index) = &mut hint.target {
                *index += final_words_count;
            }
            hint
        }));
        hypr_transcript::build_segments(&self.words, &self.partials, &all_hints, Some(&opts))
    }

    fn word_age_secs(&self, id: &str) -> f64 {
        self.word_first_seen
            .get(id)
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX)
    }

    fn apply_delta(&mut self, delta: TranscriptDelta) {
        if !delta.replaced_ids.is_empty() {
            self.words.retain(|w| !delta.replaced_ids.contains(&w.id));
            self.hints.retain(|hint| match &hint.target {
                WordRef::FinalWordId(word_id) => !delta.replaced_ids.contains(word_id),
                WordRef::RuntimeIndex(_) => true,
            });
        }
        let now = Instant::now();
        for word in &delta.new_words {
            self.word_first_seen.entry(word.id.clone()).or_insert(now);
        }
        self.words.extend(delta.new_words);
        self.hints.extend(delta.hints);
        self.partials = delta.partials;
        self.partial_hints = delta.partial_hints;
    }

    fn has_recent_words(&self) -> bool {
        let now = Instant::now();
        self.word_first_seen
            .values()
            .any(|t| now.duration_since(*t).as_secs_f64() < 0.5)
    }
}

impl Screen for RichTranscribeScreen {
    type ExternalEvent = RichEvent;
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
        event: RichEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            RichEvent::StreamResponse(response) => {
                if let Some(delta) = self.transcript.process(&response) {
                    self.apply_delta(delta);
                }
            }
            RichEvent::StreamEnded => {
                self.shell.stream_ended = true;
            }
        }
        ScreenControl::Continue
    }

    fn draw(&mut self, frame: &mut Frame) {
        let segments = self.segments();
        let content_width = frame.area().width.saturating_sub(4) as usize;
        let word_age_fn = |id: &str| self.word_age_secs(id);
        let lines = build_segment_lines(&segments, &self.theme, content_width, Some(&word_age_fn));

        self.shell.draw(
            frame,
            "Transcript",
            lines,
            "Stream ended — no speech detected.",
            self.theme.border_focused,
        );
    }

    fn title(&self) -> String {
        "debug transcribe (rich)".into()
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.has_recent_words() {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(100)
        }
    }
}
