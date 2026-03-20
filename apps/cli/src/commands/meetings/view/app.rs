use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use hypr_transcript::Segment;

use crate::commands::meetings::ui::notepad_state::{NotepadCmd, NotepadEvent, NotepadState};
use crate::theme::Theme;
use crate::widgets::ScrollViewState;

use super::action::Action;
use super::effect::Effect;

pub(crate) use crate::commands::meetings::ui::Mode;

pub(crate) struct App {
    meeting_id: String,
    title: String,
    created_at: String,
    segments: Vec<Segment>,
    notepad: NotepadState,
    loading: bool,
    error: Option<String>,
    memo_dirty: bool,
    save_message: Option<&'static str>,
    exit_after_save: bool,
}

impl App {
    pub(crate) fn new(meeting_id: String) -> Self {
        Self {
            meeting_id,
            title: String::new(),
            created_at: String::new(),
            segments: Vec::new(),
            notepad: NotepadState::new(""),
            loading: true,
            error: None,
            memo_dirty: false,
            save_message: None,
            exit_after_save: false,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::Loaded {
                meeting,
                segments,
                memo,
            } => {
                self.loading = false;
                self.title = meeting.title.unwrap_or_default();
                self.created_at = meeting.created_at;
                let memo_text = memo.as_ref().map(|n| n.content.as_str()).unwrap_or("");
                self.notepad.reset_memo(memo_text);
                self.segments = segments;
                Vec::new()
            }
            Action::LoadError(msg) => {
                self.loading = false;
                self.error = Some(msg);
                Vec::new()
            }
            Action::Saved => {
                self.memo_dirty = false;
                self.save_message = Some("saved");
                if self.exit_after_save {
                    self.exit_after_save = false;
                    vec![Effect::Exit]
                } else {
                    Vec::new()
                }
            }
            Action::SaveError(msg) => {
                self.exit_after_save = false;
                self.error = Some(format!("save failed: {msg}"));
                Vec::new()
            }
        }
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn created_at(&self) -> &str {
        &self.created_at
    }

    pub(crate) fn segments_and_scroll(&mut self) -> (&[Segment], &mut ScrollViewState) {
        (&self.segments, self.notepad.scroll_state_mut())
    }

    pub(crate) fn memo_mut(&mut self) -> &mut Editor<Theme> {
        self.notepad.memo_mut()
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

    pub(crate) fn notepad_width_percent(&self) -> u16 {
        self.notepad.notepad_width_percent()
    }

    pub(crate) fn command_buffer(&self) -> &str {
        self.notepad.command_buffer()
    }

    pub(crate) fn loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn memo_dirty(&self) -> bool {
        self.memo_dirty
    }

    pub(crate) fn save_message(&self) -> Option<&str> {
        self.save_message
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        match self.notepad.handle_key(key) {
            NotepadEvent::None => {}
            NotepadEvent::ModeChanged => {
                if self.notepad.mode() != Mode::Normal {
                    self.save_message = None;
                }
            }
            NotepadEvent::TextEdited => {
                self.memo_dirty = true;
            }
            NotepadEvent::Scrolled(_) => {}
            NotepadEvent::Command(cmd) => return self.handle_notepad_cmd(cmd),
        }

        Vec::new()
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.notepad.handle_paste(&pasted) {
            self.memo_dirty = true;
        }
        Vec::new()
    }

    fn handle_notepad_cmd(&mut self, cmd: NotepadCmd) -> Vec<Effect> {
        match cmd {
            NotepadCmd::Quit | NotepadCmd::ForceQuit => vec![Effect::Exit],
            NotepadCmd::Write => {
                vec![Effect::SaveMemo {
                    meeting_id: self.meeting_id.clone(),
                    memo: self.notepad.memo_text(),
                }]
            }
            NotepadCmd::WriteQuit => {
                self.exit_after_save = true;
                vec![Effect::SaveMemo {
                    meeting_id: self.meeting_id.clone(),
                    memo: self.notepad.memo_text(),
                }]
            }
            NotepadCmd::Unknown(cmd) => {
                self.error = Some(format!("Unknown command: :{cmd}"));
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wq_exits_only_after_successful_save() {
        let mut app = App::new("session-1".to_string());

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(':'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('w'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('q'))));
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(matches!(effects.as_slice(), [Effect::SaveMemo { .. }]));

        let effects = app.dispatch(Action::Saved);
        assert!(matches!(effects.as_slice(), [Effect::Exit]));
    }

    #[test]
    fn wq_does_not_exit_when_save_fails() {
        let mut app = App::new("session-1".to_string());

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(':'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('w'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('q'))));
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(matches!(effects.as_slice(), [Effect::SaveMemo { .. }]));

        let effects = app.dispatch(Action::SaveError("boom".to_string()));
        assert!(effects.is_empty());
        assert_eq!(app.error(), Some("save failed: boom"));
    }
}
