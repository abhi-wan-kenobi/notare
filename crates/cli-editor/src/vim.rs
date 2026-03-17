use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{Editor, KeyResult};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimMode {
    Normal,
    Insert,
}

pub(crate) struct VimState {
    pub mode: VimMode,
    pending: Option<char>,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: VimMode::Insert,
            pending: None,
        }
    }
}

impl Editor {
    pub(crate) fn handle_key_vim(&mut self, key: KeyEvent) -> KeyResult {
        match self.vim_state.mode {
            VimMode::Normal => self.handle_vim_normal(key),
            VimMode::Insert => {
                if key.code == KeyCode::Esc {
                    self.vim_state.mode = VimMode::Normal;
                    if self.cursor.col > 0 {
                        self.cursor.col -= 1;
                    }
                    return KeyResult::Consumed;
                }
                self.handle_key_insert(key)
            }
        }
    }

    fn handle_vim_normal(&mut self, key: KeyEvent) -> KeyResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('r') => {
                    self.redo();
                    KeyResult::Consumed
                }
                _ => KeyResult::Ignored,
            };
        }

        if let Some(pending) = self.vim_state.pending.take() {
            return match (pending, key.code) {
                ('d', KeyCode::Char('d')) => {
                    self.save_for_undo();
                    self.buffer.delete_line(self.cursor.row);
                    self.cursor.clamp(self.buffer.lines());
                    KeyResult::Consumed
                }
                ('g', KeyCode::Char('g')) => {
                    self.cursor.row = 0;
                    self.cursor.col = 0;
                    KeyResult::Consumed
                }
                _ => KeyResult::Consumed,
            };
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                }
                KeyResult::Consumed
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor.move_down(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor.move_up(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let max = self.buffer.line_char_count(self.cursor.row);
                if max > 0 && self.cursor.col < max - 1 {
                    self.cursor.col += 1;
                }
                KeyResult::Consumed
            }
            KeyCode::Char('w') => {
                self.cursor.move_word_forward(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('b') => {
                self.cursor.move_word_back(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('0') => {
                self.cursor.move_home();
                KeyResult::Consumed
            }
            KeyCode::Char('$') => {
                let max = self.buffer.line_char_count(self.cursor.row);
                self.cursor.col = max.saturating_sub(1);
                KeyResult::Consumed
            }
            KeyCode::Char('x') => {
                self.save_for_undo();
                self.buffer.delete_char_at(self.cursor.row, self.cursor.col);
                self.cursor.clamp(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('d') => {
                self.vim_state.pending = Some('d');
                KeyResult::Consumed
            }
            KeyCode::Char('g') => {
                self.vim_state.pending = Some('g');
                KeyResult::Consumed
            }
            KeyCode::Char('G') => {
                self.cursor.row = self.buffer.line_count().saturating_sub(1);
                self.cursor.clamp(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('u') => {
                self.undo();
                KeyResult::Consumed
            }
            KeyCode::Char('i') => {
                self.vim_state.mode = VimMode::Insert;
                KeyResult::Consumed
            }
            KeyCode::Char('a') => {
                self.vim_state.mode = VimMode::Insert;
                self.cursor.move_right(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('A') => {
                self.vim_state.mode = VimMode::Insert;
                self.cursor.move_end(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('o') => {
                self.save_for_undo();
                self.vim_state.mode = VimMode::Insert;
                self.buffer.insert_empty_line_after(self.cursor.row);
                self.cursor.row += 1;
                self.cursor.col = 0;
                self.ensure_visible();
                KeyResult::Consumed
            }
            _ => KeyResult::Ignored,
        }
    }
}
