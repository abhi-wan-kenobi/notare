use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;
use crate::widgets::ScrollViewState;

use super::Mode;

const DEFAULT_NOTEPAD_WIDTH_PERCENT: u16 = 60;
const MIN_NOTEPAD_WIDTH_PERCENT: u16 = 40;
const MAX_NOTEPAD_WIDTH_PERCENT: u16 = 75;

pub(crate) enum NotepadEvent {
    None,
    ModeChanged,
    TextEdited,
    Scrolled(ScrollDirection),
    Command(NotepadCmd),
}

pub(crate) enum ScrollDirection {
    Up,
    Down,
    Top,
    Bottom,
}

pub(crate) enum NotepadCmd {
    Quit,
    ForceQuit,
    Write,
    WriteQuit,
    Unknown(String),
}

pub(crate) struct NotepadState {
    mode: Mode,
    command_buffer: String,
    notepad_width_percent: u16,
    scroll: ScrollViewState,
    memo: Editor<Theme>,
}

impl NotepadState {
    pub(crate) fn new(initial_text: &str) -> Self {
        Self {
            mode: Mode::Normal,
            command_buffer: String::new(),
            notepad_width_percent: DEFAULT_NOTEPAD_WIDTH_PERCENT,
            scroll: ScrollViewState::new(),
            memo: Self::init_memo(initial_text),
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> NotepadEvent {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(self.mode, Mode::Normal | Mode::Insert)
        {
            match key.code {
                KeyCode::Left => {
                    self.adjust_notepad_width(-2);
                    return NotepadEvent::None;
                }
                KeyCode::Right => {
                    self.adjust_notepad_width(2);
                    return NotepadEvent::None;
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

    pub(crate) fn handle_paste(&mut self, pasted: &str) -> bool {
        if self.mode != Mode::Insert {
            return false;
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.memo.insert_str(&pasted);
        true
    }

    pub(crate) fn reset_memo(&mut self, text: &str) {
        self.memo = Self::init_memo(text);
    }

    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) fn memo_mut(&mut self) -> &mut Editor<Theme> {
        &mut self.memo
    }

    pub(crate) fn memo_text(&self) -> String {
        self.memo.text()
    }

    pub(crate) fn scroll_state_mut(&mut self) -> &mut ScrollViewState {
        &mut self.scroll
    }

    pub(crate) fn notepad_width_percent(&self) -> u16 {
        self.notepad_width_percent
    }

    pub(crate) fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> NotepadEvent {
        match key.code {
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                NotepadEvent::ModeChanged
            }
            KeyCode::Char('i') | KeyCode::Char('m') | KeyCode::Char('a') | KeyCode::Tab => {
                self.mode = Mode::Insert;
                NotepadEvent::ModeChanged
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll.scroll_down();
                NotepadEvent::Scrolled(ScrollDirection::Down)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll.scroll_up();
                NotepadEvent::Scrolled(ScrollDirection::Up)
            }
            KeyCode::Char('G') => {
                self.scroll.scroll_to_bottom();
                NotepadEvent::Scrolled(ScrollDirection::Bottom)
            }
            KeyCode::Char('g') => {
                self.scroll.scroll_to_top();
                NotepadEvent::Scrolled(ScrollDirection::Top)
            }
            _ => NotepadEvent::None,
        }
    }

    fn handle_insert_key(&mut self, key: KeyEvent) -> NotepadEvent {
        if key.code == KeyCode::Esc || key.code == KeyCode::Tab {
            self.mode = Mode::Normal;
            return NotepadEvent::ModeChanged;
        }

        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo = Self::init_memo("");
            return NotepadEvent::TextEdited;
        }

        self.memo.handle_key(key);
        NotepadEvent::TextEdited
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> NotepadEvent {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                NotepadEvent::ModeChanged
            }
            KeyCode::Enter => self.execute_command(),
            KeyCode::Backspace => {
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                    NotepadEvent::ModeChanged
                } else {
                    self.command_buffer.pop();
                    NotepadEvent::None
                }
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                NotepadEvent::None
            }
            _ => NotepadEvent::None,
        }
    }

    fn execute_command(&mut self) -> NotepadEvent {
        let cmd = self.command_buffer.trim().to_string();
        self.command_buffer.clear();
        self.mode = Mode::Normal;

        let notepad_cmd = match cmd.as_str() {
            "q" | "quit" => NotepadCmd::Quit,
            "q!" | "quit!" => NotepadCmd::ForceQuit,
            "w" | "write" => NotepadCmd::Write,
            "wq" => NotepadCmd::WriteQuit,
            _ => NotepadCmd::Unknown(cmd),
        };
        NotepadEvent::Command(notepad_cmd)
    }

    fn adjust_notepad_width(&mut self, delta: i16) {
        let next = (self.notepad_width_percent as i16 + delta).clamp(
            MIN_NOTEPAD_WIDTH_PERCENT as i16,
            MAX_NOTEPAD_WIDTH_PERCENT as i16,
        ) as u16;
        self.notepad_width_percent = next;
    }

    fn init_memo(initial: &str) -> Editor<Theme> {
        let mut memo = Editor::with_styles(Theme::DEFAULT);
        memo.set_placeholder(
            "press [i] to start writing notes...",
            Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        );
        memo.set_cursor_line_style(Style::new().add_modifier(Modifier::UNDERLINED));
        if !initial.is_empty() {
            memo.insert_str(initial);
        }
        memo
    }
}
