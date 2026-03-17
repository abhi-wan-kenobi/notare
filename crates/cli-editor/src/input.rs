use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{Editor, KeyResult};

impl Editor {
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult {
        if self.vim_enabled {
            return self.handle_key_vim(key);
        }
        self.handle_key_insert(key)
    }

    pub(crate) fn handle_key_insert(&mut self, key: KeyEvent) -> KeyResult {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if ctrl {
            return match key.code {
                KeyCode::Char('z') => {
                    self.undo();
                    KeyResult::Consumed
                }
                KeyCode::Char('y') => {
                    self.redo();
                    KeyResult::Consumed
                }
                KeyCode::Left => {
                    self.cursor.move_word_back(self.buffer.lines());
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                KeyCode::Right => {
                    self.cursor.move_word_forward(self.buffer.lines());
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                _ => KeyResult::Ignored,
            };
        }

        match key.code {
            KeyCode::Char(c) => {
                self.save_for_undo();
                let new_col = self.buffer.insert_char(self.cursor.row, self.cursor.col, c);
                self.cursor.col = new_col;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Backspace => {
                self.save_for_undo();
                if let Some((r, c)) = self
                    .buffer
                    .delete_char_before(self.cursor.row, self.cursor.col)
                {
                    self.cursor.row = r;
                    self.cursor.col = c;
                }
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Delete => {
                self.save_for_undo();
                self.buffer.delete_char_at(self.cursor.row, self.cursor.col);
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Left => {
                self.cursor.move_left(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Right => {
                self.cursor.move_right(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Up if !self.single_line => {
                self.cursor.move_up(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Down if !self.single_line => {
                self.cursor.move_down(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Home => {
                self.cursor.move_home();
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::End => {
                self.cursor.move_end(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Enter if !self.single_line => {
                self.save_for_undo();
                let (r, c) = self.buffer.insert_newline(self.cursor.row, self.cursor.col);
                self.cursor.row = r;
                self.cursor.col = c;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Tab if !self.single_line => {
                self.save_for_undo();
                let new_col = self
                    .buffer
                    .insert_char(self.cursor.row, self.cursor.col, '\t');
                self.cursor.col = new_col;
                self.ensure_visible();
                KeyResult::Consumed
            }
            _ => KeyResult::Ignored,
        }
    }
}
