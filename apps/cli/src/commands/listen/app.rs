use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_listener_core::{
    DegradedError, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, State,
};
use hypr_listener2_core::BatchEvent;
use hypr_transcript::{
    FinalizedWord, PartialWord, RuntimeSpeakerHint, Segment, TranscriptDelta, TranscriptProcessor,
    WordRef,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Block;
use tachyonfx::{Effect, Interpolation, Motion, fx};
use tui_textarea::TextArea;

use super::audio_drop::{AudioDropRequest, looks_like_audio_file, normalize_pasted_path};
use super::runtime::ListenerEvent;
use crate::frame::FrameRequester;
use crate::textarea_input::textarea_input_from_key_event;

const AUDIO_HISTORY_CAP: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

pub struct App {
    pub should_quit: bool,
    pub force_quit: bool,
    pub state: State,
    pub status: String,
    pub degraded: Option<DegradedError>,
    pub errors: Vec<String>,
    pub mic_level: u16,
    pub speaker_level: u16,
    pub mic_history: VecDeque<u64>,
    pub speaker_history: VecDeque<u64>,
    pub mic_muted: bool,
    pub words: Vec<FinalizedWord>,
    pub partials: Vec<PartialWord>,
    pub hints: Vec<RuntimeSpeakerHint>,
    pub partial_hints: Vec<RuntimeSpeakerHint>,
    transcript: TranscriptProcessor,
    pub started_at: std::time::Instant,
    pub scroll_offset: u16,
    frame_requester: FrameRequester,

    mode: Mode,
    pub command_buffer: String,
    notepad_width_percent: u16,
    transcript_max_scroll: u16,
    transcript_autoscroll: bool,
    memo: TextArea<'static>,
    batch_running: bool,

    // Animation state
    word_first_seen: HashMap<String, Instant>,
    last_frame_time: Instant,
    prev_segment_count: usize,
    pub transcript_effects: Vec<Effect>,
}

impl App {
    fn init_memo() -> TextArea<'static> {
        let mut memo = TextArea::default();
        memo.set_placeholder_text("press [i] to start writing notes...");
        memo.set_placeholder_style(
            Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        );
        memo.set_cursor_line_style(Style::new().add_modifier(Modifier::UNDERLINED));
        memo
    }

    pub fn new(frame_requester: FrameRequester) -> Self {
        let now = Instant::now();
        Self {
            should_quit: false,
            force_quit: false,
            state: State::Inactive,
            status: "Starting...".into(),
            degraded: None,
            errors: Vec::new(),
            mic_level: 0,
            speaker_level: 0,
            mic_history: VecDeque::with_capacity(AUDIO_HISTORY_CAP),
            speaker_history: VecDeque::with_capacity(AUDIO_HISTORY_CAP),
            mic_muted: false,
            words: Vec::new(),
            partials: Vec::new(),
            hints: Vec::new(),
            partial_hints: Vec::new(),
            transcript: TranscriptProcessor::new(),
            started_at: now,
            scroll_offset: 0,
            frame_requester,

            mode: Mode::Normal,
            command_buffer: String::new(),
            notepad_width_percent: 60,
            transcript_max_scroll: 0,
            transcript_autoscroll: true,
            memo: Self::init_memo(),
            batch_running: false,

            word_first_seen: HashMap::new(),
            last_frame_time: now,
            prev_segment_count: 0,
            transcript_effects: Vec::new(),
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(self.mode, Mode::Normal | Mode::Insert)
        {
            match key.code {
                KeyCode::Left => {
                    self.adjust_notepad_width(-2);
                    self.frame_requester.schedule_frame();
                    return;
                }
                KeyCode::Right => {
                    self.adjust_notepad_width(2);
                    self.frame_requester.schedule_frame();
                    return;
                }
                _ => {}
            }
        }

        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Insert => self.handle_insert_key(key),
            Mode::Command => self.handle_command_key(key),
        }
    }

    pub fn handle_paste(&mut self, pasted: String) -> Option<AudioDropRequest> {
        if self.mode != Mode::Insert {
            return self.handle_transcript_paste(pasted);
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.memo.insert_str(&pasted);
        self.frame_requester.schedule_frame();
        None
    }

    pub fn handle_listener_event(&mut self, event: ListenerEvent) {
        match event {
            ListenerEvent::Lifecycle(e) => self.handle_lifecycle(e),
            ListenerEvent::Progress(e) => self.handle_progress(e),
            ListenerEvent::Error(e) => self.handle_error(e),
            ListenerEvent::Data(e) => self.handle_data(e),
        }
        self.frame_requester.schedule_frame();
    }

    pub fn handle_batch_event(&mut self, event: BatchEvent) {
        match event {
            BatchEvent::BatchStarted { .. } => {
                self.batch_running = true;
                self.status = "Transcribing dropped audio...".into();
            }
            BatchEvent::BatchCompleted { .. } => {
                self.batch_running = false;
                self.status = "Dropped audio transcription completed".into();
            }
            BatchEvent::BatchResponseStreamed {
                response,
                percentage,
                ..
            } => {
                if let Some(delta) = self.transcript.process(&response) {
                    self.apply_transcript_delta(delta);
                }

                self.status = format!("Transcribing dropped audio... {:.0}%", percentage * 100.0);

                if percentage >= 1.0 {
                    self.batch_running = false;
                    self.status = "Dropped audio transcription completed".into();
                }
            }
            BatchEvent::BatchResponse { response, .. } => {
                let delta = TranscriptProcessor::process_batch_response(&response);
                self.apply_transcript_delta(delta);
                self.batch_running = false;
                self.status = "Dropped audio transcription completed".into();
            }
            BatchEvent::BatchFailed { error, .. } => {
                self.batch_running = false;
                self.errors.push(format!("Batch: {error}"));
                self.status = format!("Dropped audio transcription failed: {error}");
            }
        }

        self.frame_requester.schedule_frame();
    }

    pub fn can_accept_audio_drop(&self) -> bool {
        self.mode == Mode::Normal
            && self.state == State::Inactive
            && !self.batch_running
            && self.words.is_empty()
            && self.partials.is_empty()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn memo_focused(&self) -> bool {
        self.mode == Mode::Insert
    }

    pub fn transcript_focused(&self) -> bool {
        self.mode == Mode::Normal
    }

    pub fn set_memo_block(&mut self, block: Block<'static>) {
        self.memo.set_block(block);
    }

    pub fn memo(&self) -> &TextArea<'static> {
        &self.memo
    }

    pub fn update_transcript_max_scroll(&mut self, max_scroll: u16) {
        self.transcript_max_scroll = max_scroll;
        if self.transcript_autoscroll {
            self.scroll_offset = max_scroll;
        } else {
            self.scroll_offset = self.scroll_offset.min(max_scroll);
        }
    }

    pub fn notepad_width_percent(&self) -> u16 {
        self.notepad_width_percent
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('i') | KeyCode::Char('m') | KeyCode::Char('a') => {
                self.mode = Mode::Insert;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Tab => {
                self.mode = Mode::Insert;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(1)
                    .min(self.transcript_max_scroll);
                self.transcript_autoscroll = self.scroll_offset >= self.transcript_max_scroll;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                self.transcript_autoscroll = false;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('G') => {
                self.scroll_offset = self.transcript_max_scroll;
                self.transcript_autoscroll = true;
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char('g') => {
                self.scroll_offset = 0;
                self.transcript_autoscroll = false;
                self.frame_requester.schedule_frame();
            }
            _ => {}
        }
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Tab {
            self.mode = Mode::Normal;
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo = Self::init_memo();
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo.undo();
            self.frame_requester.schedule_frame();
            return;
        }

        if key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo.redo();
            self.frame_requester.schedule_frame();
            return;
        }

        if let Some(input) = textarea_input_from_key_event(key, true) {
            self.memo.input(input);
        }

        self.frame_requester.schedule_frame();
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.frame_requester.schedule_frame();
            }
            KeyCode::Enter => {
                self.execute_command();
                self.frame_requester.schedule_frame();
            }
            KeyCode::Backspace => {
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.command_buffer.pop();
                }
                self.frame_requester.schedule_frame();
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                self.frame_requester.schedule_frame();
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.command_buffer.trim().to_string();
        self.command_buffer.clear();
        self.mode = Mode::Normal;

        match cmd.as_str() {
            "q" | "quit" => {
                self.should_quit = true;
            }
            "q!" | "quit!" => {
                self.force_quit = true;
                self.should_quit = true;
            }
            _ => {
                self.errors.push(format!("Unknown command: :{cmd}"));
            }
        }
    }

    fn adjust_notepad_width(&mut self, delta: i16) {
        const MIN_NOTEPAD_WIDTH_PERCENT: u16 = 40;
        const MAX_NOTEPAD_WIDTH_PERCENT: u16 = 75;

        let next = (self.notepad_width_percent as i16 + delta).clamp(
            MIN_NOTEPAD_WIDTH_PERCENT as i16,
            MAX_NOTEPAD_WIDTH_PERCENT as i16,
        ) as u16;
        self.notepad_width_percent = next;
    }

    fn handle_lifecycle(&mut self, event: SessionLifecycleEvent) {
        match event {
            SessionLifecycleEvent::Active { error, .. } => {
                self.state = State::Active;
                self.degraded = error;
                if self.degraded.is_some() {
                    self.status = "Active (degraded)".into();
                } else {
                    self.status = "Listening".into();
                }
            }
            SessionLifecycleEvent::Inactive { error, .. } => {
                self.state = State::Inactive;
                if let Some(err) = error {
                    self.status = format!("Stopped: {err}");
                } else {
                    self.status = "Stopped".into();
                }
            }
            SessionLifecycleEvent::Finalizing { .. } => {
                self.state = State::Finalizing;
                self.status = "Finalizing...".into();
            }
        }
    }

    fn handle_progress(&mut self, event: SessionProgressEvent) {
        match event {
            SessionProgressEvent::AudioInitializing { .. } => {
                self.status = "Initializing audio...".into();
            }
            SessionProgressEvent::AudioReady { device, .. } => {
                if let Some(dev) = device {
                    self.status = format!("Audio ready ({dev})");
                } else {
                    self.status = "Audio ready".into();
                }
            }
            SessionProgressEvent::Connecting { .. } => {
                self.status = "Connecting...".into();
            }
            SessionProgressEvent::Connected { adapter, .. } => {
                self.status = format!("Connected via {adapter}");
            }
        }
    }

    fn handle_error(&mut self, event: SessionErrorEvent) {
        match event {
            SessionErrorEvent::AudioError { error, .. } => {
                self.errors.push(format!("Audio: {error}"));
            }
            SessionErrorEvent::ConnectionError { error, .. } => {
                self.errors.push(format!("Connection: {error}"));
            }
        }
    }

    fn handle_data(&mut self, event: SessionDataEvent) {
        match event {
            SessionDataEvent::AudioAmplitude { mic, speaker, .. } => {
                self.mic_level = mic;
                self.speaker_level = speaker;

                if self.mic_history.len() >= AUDIO_HISTORY_CAP {
                    self.mic_history.pop_front();
                }
                self.mic_history.push_back(mic as u64);

                if self.speaker_history.len() >= AUDIO_HISTORY_CAP {
                    self.speaker_history.pop_front();
                }
                self.speaker_history.push_back(speaker as u64);
            }
            SessionDataEvent::MicMuted { value, .. } => {
                self.mic_muted = value;
            }
            SessionDataEvent::StreamResponse { response, .. } => {
                if let Some(delta) = self.transcript.process(response.as_ref()) {
                    self.apply_transcript_delta(delta);
                }
            }
        }
    }

    fn apply_transcript_delta(&mut self, delta: TranscriptDelta) {
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

    pub fn segments(&self) -> Vec<Segment> {
        let opts = hypr_transcript::SegmentBuilderOptions {
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

    pub fn word_age_secs(&self, id: &str) -> f64 {
        self.word_first_seen
            .get(id)
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX)
    }

    pub fn frame_elapsed(&mut self) -> std::time::Duration {
        let now = Instant::now();
        let elapsed = now - self.last_frame_time;
        self.last_frame_time = now;
        elapsed
    }

    pub fn check_new_segments(&mut self, current_count: usize, transcript_area: Rect) {
        if current_count > self.prev_segment_count && self.prev_segment_count > 0 {
            let effect = fx::sweep_in(
                Motion::LeftToRight,
                8,
                0,
                Color::Rgb(0, 60, 80),
                (350u32, Interpolation::CubicOut),
            )
            .with_area(transcript_area);
            self.transcript_effects.push(effect);
        }
        self.prev_segment_count = current_count;
    }

    pub fn process_effects(
        &mut self,
        elapsed: std::time::Duration,
        buf: &mut ratatui::buffer::Buffer,
        area: Rect,
    ) {
        let elapsed: tachyonfx::Duration = elapsed.into();
        self.transcript_effects.retain_mut(|effect| {
            effect.process(elapsed, buf, area);
            !effect.done()
        });
    }

    pub fn has_active_animations(&self) -> bool {
        if !self.transcript_effects.is_empty() {
            return true;
        }
        let now = Instant::now();
        self.word_first_seen
            .values()
            .any(|t| now.duration_since(*t).as_secs_f64() < 0.5)
    }

    fn handle_transcript_paste(&mut self, pasted: String) -> Option<AudioDropRequest> {
        if !self.can_accept_audio_drop() {
            return None;
        }

        let path = normalize_pasted_path(&pasted)?;

        if !looks_like_audio_file(&path) {
            return None;
        }

        if !path.is_file() {
            self.errors
                .push(format!("Dropped path is not a file: {}", path.display()));
            self.frame_requester.schedule_frame();
            return None;
        }

        self.batch_running = true;
        self.status = format!("Transcribing dropped audio: {}", path.display());
        self.frame_requester.schedule_frame();

        Some(AudioDropRequest {
            file_path: path.to_string_lossy().to_string(),
        })
    }
}
