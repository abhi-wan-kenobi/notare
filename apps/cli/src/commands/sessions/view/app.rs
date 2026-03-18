use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::Editor;
use hypr_transcript::Segment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Block;

use crate::theme::Theme;
use crate::widgets::ScrollViewState;

use super::action::Action;
use super::effect::Effect;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    Insert,
    Command,
}

const DEFAULT_NOTEPAD_WIDTH_PERCENT: u16 = 60;
const MIN_NOTEPAD_WIDTH_PERCENT: u16 = 40;
const MAX_NOTEPAD_WIDTH_PERCENT: u16 = 75;

pub(crate) struct App {
    session_id: String,
    title: String,
    created_at: String,
    segments: Vec<Segment>,
    memo: Editor<Theme>,
    mode: Mode,
    scroll: ScrollViewState,
    command_buffer: String,
    notepad_width_percent: u16,
    loading: bool,
    error: Option<String>,
    memo_dirty: bool,
    save_message: Option<&'static str>,
}

impl App {
    pub(crate) fn new(session_id: String) -> Self {
        Self {
            session_id,
            title: String::new(),
            created_at: String::new(),
            segments: Vec::new(),
            memo: Self::init_memo(""),
            mode: Mode::Normal,
            scroll: ScrollViewState::new(),
            command_buffer: String::new(),
            notepad_width_percent: DEFAULT_NOTEPAD_WIDTH_PERCENT,
            loading: true,
            error: None,
            memo_dirty: false,
            save_message: None,
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::Loaded { session, segments } => {
                self.loading = false;
                self.title = session.title.unwrap_or_default();
                self.created_at = session.created_at;
                let memo_text = session.memo.as_deref().unwrap_or("");
                self.memo = Self::init_memo(memo_text);
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
                Vec::new()
            }
            Action::SaveError(msg) => {
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

    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub(crate) fn memo(&self) -> &Editor<Theme> {
        &self.memo
    }

    pub(crate) fn set_memo_block(&mut self, block: Block<'static>) {
        self.memo.set_block(block);
    }

    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) fn memo_focused(&self) -> bool {
        self.mode == Mode::Insert
    }

    pub(crate) fn transcript_focused(&self) -> bool {
        self.mode == Mode::Normal
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

    fn memo_text(&self) -> String {
        self.memo.lines().join("\n")
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(self.mode, Mode::Normal | Mode::Insert)
        {
            match key.code {
                KeyCode::Left => {
                    self.adjust_notepad_width(-2);
                    return Vec::new();
                }
                KeyCode::Right => {
                    self.adjust_notepad_width(2);
                    return Vec::new();
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

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        if self.mode != Mode::Insert {
            return Vec::new();
        }
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.memo.insert_str(&pasted);
        self.memo_dirty = true;
        Vec::new()
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.save_message = None;
            }
            KeyCode::Char('i') | KeyCode::Char('a') | KeyCode::Tab => {
                self.mode = Mode::Insert;
                self.save_message = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll.scroll_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll.scroll_up();
            }
            KeyCode::Char('G') => {
                self.scroll.scroll_to_bottom();
            }
            KeyCode::Char('g') => {
                self.scroll.scroll_to_top();
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_insert_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc || key.code == KeyCode::Tab {
            self.mode = Mode::Normal;
            return Vec::new();
        }

        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.memo = Self::init_memo("");
            self.memo_dirty = true;
            return Vec::new();
        }

        self.memo.handle_key(key);
        self.memo_dirty = true;
        Vec::new()
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Enter => {
                return self.execute_command();
            }
            KeyCode::Backspace => {
                if self.command_buffer.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.command_buffer.pop();
                }
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            _ => {}
        }
        Vec::new()
    }

    fn execute_command(&mut self) -> Vec<Effect> {
        let cmd = self.command_buffer.trim().to_string();
        self.command_buffer.clear();
        self.mode = Mode::Normal;

        match cmd.as_str() {
            "q" | "quit" => {
                vec![Effect::Exit]
            }
            "q!" | "quit!" => {
                vec![Effect::Exit]
            }
            "w" | "write" => {
                vec![Effect::SaveMemo {
                    session_id: self.session_id.clone(),
                    memo: self.memo_text(),
                }]
            }
            "wq" => {
                vec![
                    Effect::SaveMemo {
                        session_id: self.session_id.clone(),
                        memo: self.memo_text(),
                    },
                    Effect::Exit,
                ]
            }
            _ => {
                self.error = Some(format!("Unknown command: :{cmd}"));
                Vec::new()
            }
        }
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
