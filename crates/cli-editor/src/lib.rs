mod buffer;
mod cursor;
mod highlight;
mod history;
mod input;
mod render;
mod vim;

use std::cell::Cell;

use ratatui::style::Style;
use ratatui::widgets::Block;

use buffer::Buffer;
use cursor::Cursor;
use highlight::Highlighter;
use history::{History, Snapshot};
use vim::{VimMode, VimState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    Consumed,
    Ignored,
}

pub struct Editor {
    buffer: Buffer,
    cursor: Cursor,
    history: History,
    single_line: bool,
    block: Option<Block<'static>>,
    placeholder_text: Option<String>,
    placeholder_style: Style,
    cursor_line_style: Style,
    scroll_offset: Cell<usize>,
    last_known_height: Cell<u16>,
    vim_enabled: bool,
    vim_state: VimState,
    highlight_enabled: bool,
    highlighter: Highlighter,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            history: History::new(),
            single_line: false,
            block: None,
            placeholder_text: None,
            placeholder_style: Style::default(),
            cursor_line_style: Style::default(),
            scroll_offset: Cell::new(0),
            last_known_height: Cell::new(0),
            vim_enabled: false,
            vim_state: VimState::new(),
            highlight_enabled: true,
            highlighter: Highlighter::new(),
        }
    }

    pub fn single_line() -> Self {
        Self {
            single_line: true,
            highlight_enabled: false,
            ..Self::new()
        }
    }

    pub fn from_lines(lines: Vec<String>) -> Self {
        let mut editor = Self::new();
        editor.buffer = Buffer::from_lines(lines);
        let last = editor.buffer.line_count().saturating_sub(1);
        editor.cursor.row = last;
        editor.cursor.col = editor.buffer.line_char_count(last);
        editor
    }

    // Input

    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.save_for_undo();
        let text = if self.single_line {
            match s.lines().next() {
                Some(line) => line.to_string(),
                None => return,
            }
        } else {
            s.to_string()
        };
        if text.is_empty() {
            return;
        }
        let (r, c) = self
            .buffer
            .insert_str_at(self.cursor.row, self.cursor.col, &text);
        self.cursor.row = r;
        self.cursor.col = c;
        self.ensure_visible();
    }

    // Read state

    pub fn lines(&self) -> &[String] {
        self.buffer.lines()
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor.row, self.cursor.col)
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    // Undo/redo

    pub fn undo(&mut self) -> bool {
        let current = Snapshot {
            lines: self.buffer.snapshot(),
            row: self.cursor.row,
            col: self.cursor.col,
        };
        if let Some(prev) = self.history.undo(current) {
            self.buffer.restore(prev.lines);
            self.cursor.row = prev.row;
            self.cursor.col = prev.col;
            self.cursor.clamp(self.buffer.lines());
            self.ensure_visible();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        let current = Snapshot {
            lines: self.buffer.snapshot(),
            row: self.cursor.row,
            col: self.cursor.col,
        };
        if let Some(next) = self.history.redo(current) {
            self.buffer.restore(next.lines);
            self.cursor.row = next.row;
            self.cursor.col = next.col;
            self.cursor.clamp(self.buffer.lines());
            self.ensure_visible();
            true
        } else {
            false
        }
    }

    // Appearance

    pub fn set_block(&mut self, block: Block<'static>) {
        self.block = Some(block);
    }

    pub fn set_placeholder(&mut self, text: &str, style: Style) {
        self.placeholder_text = Some(text.to_string());
        self.placeholder_style = style;
    }

    pub fn set_cursor_line_style(&mut self, style: Style) {
        self.cursor_line_style = style;
    }

    // Vim

    pub fn set_vim_mode(&mut self, enabled: bool) {
        self.vim_enabled = enabled;
        if enabled {
            self.vim_state = VimState::new();
        }
    }

    pub fn vim_mode_label(&self) -> Option<&str> {
        if !self.vim_enabled {
            return None;
        }
        Some(match self.vim_state.mode {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
        })
    }

    // Highlighting

    pub fn set_highlight(&mut self, enabled: bool) {
        self.highlight_enabled = enabled;
    }

    // Internal

    pub(crate) fn save_for_undo(&mut self) {
        self.history.push(Snapshot {
            lines: self.buffer.snapshot(),
            row: self.cursor.row,
            col: self.cursor.col,
        });
    }

    pub(crate) fn ensure_visible(&self) {
        let height = self.last_known_height.get() as usize;
        if height == 0 {
            return;
        }
        let mut scroll = self.scroll_offset.get();
        if self.cursor.row < scroll {
            scroll = self.cursor.row;
        } else if self.cursor.row >= scroll + height {
            scroll = self.cursor.row - height + 1;
        }
        self.scroll_offset.set(scroll);
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
