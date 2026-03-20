use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use super::list::ModelRow;

pub(crate) struct App {
    models: Vec<ModelRow>,
    list_state: ListState,
    loading: bool,
    error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            list_state: ListState::default(),
            loading: true,
            error: None,
        }
    }

    pub fn set_models(&mut self, models: Vec<ModelRow>) {
        self.loading = false;
        self.models = models;
        if !self.models.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn models(&self) -> &[ModelRow] {
        &self.models
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn loading(&self) -> bool {
        self.loading
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return true;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.list_state.select_previous();
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list_state.select_next();
                false
            }
            KeyCode::Char('q') => true,
            _ => false,
        }
    }
}
