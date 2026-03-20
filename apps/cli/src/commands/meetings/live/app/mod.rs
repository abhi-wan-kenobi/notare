mod state;

use std::collections::HashMap;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use hypr_transcript::SpeakerLabelContext;

use crate::commands::meetings::ui::notepad_state::{
    NotepadCmd, NotepadEvent, NotepadState, ScrollDirection,
};
use crate::theme::Theme;
use hypr_listener_core::State;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{Interpolation, Motion, fx};

use self::state::LiveState;
use super::action::Action;
use super::effect::Effect;
pub(crate) use crate::commands::meetings::ui::Mode;

pub(crate) struct App {
    state: LiveState,
    notepad: NotepadState,
    participant_names: HashMap<String, String>,
    transcript_autoscroll: bool,
    last_frame_time: Instant,
    prev_segment_count: usize,
    transcript_effects: Vec<tachyonfx::Effect>,
}

impl App {
    pub(crate) fn new(participant_names: HashMap<String, String>) -> Self {
        Self {
            state: LiveState::new(),
            notepad: NotepadState::new(""),
            participant_names,
            transcript_autoscroll: true,
            last_frame_time: Instant::now(),
            prev_segment_count: 0,
            transcript_effects: Vec::new(),
        }
    }

    pub(crate) fn speaker_label_context(&self) -> Option<SpeakerLabelContext> {
        if self.participant_names.is_empty() {
            return None;
        }
        Some(SpeakerLabelContext {
            self_human_id: None,
            human_name_by_id: self.participant_names.clone(),
        })
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.state.elapsed()
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::RuntimeEvent(event) => {
                self.state.handle_runtime_event(event);
                Vec::new()
            }
        }
    }

    pub(crate) fn apply_runtime_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = super::runtime::RuntimeEvent>,
    {
        self.state.apply_runtime_events(events);
    }

    pub(crate) fn mode(&self) -> Mode {
        self.notepad.mode()
    }

    pub(crate) fn memo_focused(&self) -> bool {
        self.notepad.mode() == Mode::Insert
    }

    pub(crate) fn transcript_focused(&self) -> bool {
        self.notepad.mode() == Mode::Normal
    }

    pub(crate) fn memo_mut(&mut self) -> &mut Editor<Theme> {
        self.notepad.memo_mut()
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut crate::widgets::ScrollViewState {
        if self.transcript_autoscroll {
            self.notepad.scroll_state_mut().scroll_to_bottom();
        }
        self.notepad.scroll_state_mut()
    }

    pub(crate) fn notepad_width_percent(&self) -> u16 {
        self.notepad.notepad_width_percent()
    }

    pub(crate) fn listener_state(&self) -> State {
        self.state.listener_state()
    }

    pub(crate) fn status(&self) -> &str {
        self.state.status()
    }

    pub(crate) fn degraded(&self) -> Option<&hypr_listener_core::DegradedError> {
        self.state.degraded()
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.state.last_error()
    }

    pub(crate) fn mic_muted(&self) -> bool {
        self.state.mic_muted()
    }

    pub(crate) fn mic_history(&self) -> &std::collections::VecDeque<u64> {
        self.state.mic_history()
    }

    pub(crate) fn speaker_history(&self) -> &std::collections::VecDeque<u64> {
        self.state.speaker_history()
    }

    pub(crate) fn word_count(&self) -> usize {
        self.state.word_count()
    }

    pub(crate) fn words(&self) -> Vec<hypr_transcript::FinalizedWord> {
        self.state.words().to_vec()
    }

    pub(crate) fn hints(&self) -> Vec<hypr_transcript::RuntimeSpeakerHint> {
        self.state.hints().to_vec()
    }

    pub(crate) fn memo_text(&self) -> String {
        self.notepad.memo_text()
    }

    pub(crate) fn command_buffer(&self) -> &str {
        self.notepad.command_buffer()
    }

    pub(crate) fn segments(&self) -> Vec<hypr_transcript::Segment> {
        self.state.segments()
    }

    pub(crate) fn word_age_secs(&self, id: &str) -> f64 {
        self.state.word_age_secs(id)
    }

    pub(crate) fn frame_elapsed(&mut self) -> std::time::Duration {
        let now = Instant::now();
        let elapsed = now - self.last_frame_time;
        self.last_frame_time = now;
        elapsed
    }

    pub(crate) fn check_new_segments(&mut self, current_count: usize, transcript_area: Rect) {
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

    pub(crate) fn process_effects(
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

    pub(crate) fn has_active_animations(&self) -> bool {
        !self.transcript_effects.is_empty() || self.state.has_recent_words()
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit { force: false }];
        }

        match self.notepad.handle_key(key) {
            NotepadEvent::None => {}
            NotepadEvent::ModeChanged => {}
            NotepadEvent::TextEdited => {}
            NotepadEvent::Scrolled(dir) => self.update_autoscroll(dir),
            NotepadEvent::Command(cmd) => return self.handle_notepad_cmd(cmd),
        }

        Vec::new()
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        self.notepad.handle_paste(&pasted);
        Vec::new()
    }

    fn handle_notepad_cmd(&mut self, cmd: NotepadCmd) -> Vec<Effect> {
        match cmd {
            NotepadCmd::Quit => vec![Effect::Exit { force: false }],
            NotepadCmd::ForceQuit => vec![Effect::Exit { force: true }],
            NotepadCmd::Write => {
                self.state.push_error("Unknown command: :w".to_string());
                Vec::new()
            }
            NotepadCmd::WriteQuit => {
                self.state.push_error("Unknown command: :wq".to_string());
                Vec::new()
            }
            NotepadCmd::Unknown(s) => {
                self.state.push_error(format!("Unknown command: :{s}"));
                Vec::new()
            }
        }
    }

    fn update_autoscroll(&mut self, dir: ScrollDirection) {
        match dir {
            ScrollDirection::Up | ScrollDirection::Top => {
                self.transcript_autoscroll = false;
            }
            ScrollDirection::Bottom => {
                self.transcript_autoscroll = true;
            }
            ScrollDirection::Down => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_exits_without_force() {
        let mut app = App::new(HashMap::new());

        let effects = app.dispatch(Action::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));

        assert!(matches!(
            effects.as_slice(),
            [Effect::Exit { force: false }]
        ));
    }
}
