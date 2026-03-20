use crossterm::event::{KeyCode, KeyModifiers};

pub(crate) struct App {
    pub(crate) current: String,
    pub(crate) latest: String,
    pub(crate) update_command: String,
    pub(crate) selected: usize,
}

const ITEM_COUNT: usize = 3;

pub(crate) enum Outcome {
    Continue,
    AcceptUpdate,
    Skip,
    SkipVersion,
}

impl App {
    pub(crate) fn new(current: String, latest: String, update_command: String) -> Self {
        Self {
            current,
            latest,
            update_command,
            selected: 0,
        }
    }

    pub(crate) fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Outcome {
        match (key.code, key.modifiers) {
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                self.selected = self.selected.saturating_sub(1);
                Outcome::Continue
            }
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                if self.selected + 1 < ITEM_COUNT {
                    self.selected += 1;
                }
                Outcome::Continue
            }
            (KeyCode::Enter, _) => match self.selected {
                0 => Outcome::AcceptUpdate,
                1 => Outcome::Skip,
                2 => Outcome::SkipVersion,
                _ => Outcome::Continue,
            },
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => Outcome::Skip,
            _ => Outcome::Continue,
        }
    }
}
