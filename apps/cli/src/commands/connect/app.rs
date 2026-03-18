use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use url::Url;

use crate::cli::{ConnectProvider, ConnectionType};

use super::action::Action;
use super::effect::{Effect, SaveData};
use super::providers::{LLM_PROVIDERS, STT_PROVIDERS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    SelectType,
    SelectProvider,
    InputBaseUrl,
    InputApiKey,
    Done,
}

pub(crate) struct App {
    step: Step,
    connection_type: Option<ConnectionType>,
    provider: Option<ConnectProvider>,
    base_url: Option<String>,
    api_key: Option<String>,
    current_llm_provider: Option<String>,
    current_stt_provider: Option<String>,
    list_state: ListState,
    input: String,
    cursor_pos: usize,
    input_default: Option<String>,
    input_label: &'static str,
    input_masked: bool,
    error: Option<String>,
}

impl App {
    pub fn new(
        connection_type: Option<ConnectionType>,
        provider: Option<ConnectProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
        current_llm_provider: Option<String>,
        current_stt_provider: Option<String>,
    ) -> (Self, Vec<Effect>) {
        let mut app = Self {
            step: Step::SelectType,
            connection_type,
            provider,
            base_url,
            api_key,
            current_llm_provider,
            current_stt_provider,
            list_state: ListState::default(),
            input: String::new(),
            cursor_pos: 0,
            input_default: None,
            input_label: "",
            input_masked: false,
            error: None,
        };
        let effects = app.advance();
        (app, effects)
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(text) => self.handle_paste(&text),
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn provider(&self) -> Option<ConnectProvider> {
        self.provider
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    pub fn input_default(&self) -> Option<&str> {
        self.input_default.as_deref()
    }

    pub fn input_label(&self) -> &'static str {
        self.input_label
    }

    pub fn input_masked(&self) -> bool {
        self.input_masked
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn provider_entries(&self) -> &'static [ConnectProvider] {
        match self.connection_type {
            Some(ConnectionType::Llm) => LLM_PROVIDERS,
            Some(ConnectionType::Stt) => STT_PROVIDERS,
            None => &[],
        }
    }

    pub fn current_llm_provider(&self) -> Option<&str> {
        self.current_llm_provider.as_deref()
    }

    pub fn current_stt_provider(&self) -> Option<&str> {
        self.current_stt_provider.as_deref()
    }

    pub fn breadcrumb(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ct) = self.connection_type {
            parts.push(ct.to_string().to_uppercase());
        }
        if let Some(p) = self.provider {
            parts.push(p.to_string());
        }
        parts.join(" > ")
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return vec![Effect::Exit];
        }

        match self.step {
            Step::SelectType | Step::SelectProvider => self.handle_list_key(key),
            Step::InputBaseUrl | Step::InputApiKey => self.handle_input_key(key),
            Step::Done => Vec::new(),
        }
    }

    fn handle_paste(&mut self, text: &str) -> Vec<Effect> {
        match self.step {
            Step::InputBaseUrl | Step::InputApiKey => {
                for c in text.chars() {
                    let idx = self.byte_index();
                    self.input.insert(idx, c);
                    self.cursor_pos += 1;
                }
                self.error = None;
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let len = match self.step {
            Step::SelectType => 2,
            Step::SelectProvider => self.provider_entries().len(),
            _ => return Vec::new(),
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.list_navigate(-1, len);
                Vec::new()
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list_navigate(1, len);
                Vec::new()
            }
            KeyCode::Enter => {
                match self.step {
                    Step::SelectType => {
                        self.confirm_type_selection();
                        self.step = Step::SelectProvider;
                    }
                    Step::SelectProvider => {
                        self.confirm_provider_selection();
                        self.step = Step::InputBaseUrl;
                    }
                    _ => unreachable!(),
                }
                self.advance()
            }
            KeyCode::Char('q') => vec![Effect::Exit],
            _ => Vec::new(),
        }
    }

    fn list_navigate(&mut self, direction: isize, len: usize) {
        let current = self.list_state.selected().unwrap_or(0);
        let next = current as isize + direction;
        if next >= 0 && (next as usize) < len {
            self.list_state.select(Some(next as usize));
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Enter => {
                if let Err(msg) = self.confirm_input() {
                    self.error = Some(msg);
                    return Vec::new();
                }
                self.error = None;
                self.step = match self.step {
                    Step::InputBaseUrl => Step::InputApiKey,
                    Step::InputApiKey => Step::Done,
                    _ => unreachable!(),
                };
                self.advance()
            }
            KeyCode::Char(c) => {
                let idx = self.byte_index();
                self.input.insert(idx, c);
                self.cursor_pos += 1;
                self.error = None;
                Vec::new()
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let idx = self.byte_index();
                    self.input.remove(idx);
                }
                self.error = None;
                Vec::new()
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Right => {
                let max = self.input.chars().count();
                if self.cursor_pos < max {
                    self.cursor_pos += 1;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_pos)
            .unwrap_or(self.input.len())
    }

    fn confirm_type_selection(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        self.connection_type = Some(match idx {
            0 => ConnectionType::Llm,
            _ => ConnectionType::Stt,
        });
    }

    fn confirm_provider_selection(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        let providers = match self.connection_type {
            Some(ConnectionType::Llm) => LLM_PROVIDERS,
            Some(ConnectionType::Stt) => STT_PROVIDERS,
            None => return,
        };
        if let Some(&provider) = providers.get(idx) {
            self.provider = Some(provider);
        }
    }

    fn confirm_input(&mut self) -> Result<(), String> {
        let value = if self.input.trim().is_empty() {
            self.input_default.clone()
        } else {
            Some(self.input.trim().to_string())
        };

        match self.step {
            Step::InputBaseUrl => {
                if let Some(ref url) = value {
                    validate_base_url(url)?;
                }
                self.base_url = value;
            }
            Step::InputApiKey => {
                self.api_key = value;
            }
            _ => {}
        }
        Ok(())
    }

    fn advance(&mut self) -> Vec<Effect> {
        loop {
            match self.step {
                Step::SelectType => {
                    if self.connection_type.is_some() {
                        self.step = Step::SelectProvider;
                        continue;
                    }
                    self.list_state = ListState::default().with_selected(Some(0));
                    return Vec::new();
                }
                Step::SelectProvider => {
                    if let Some(provider) = self.provider {
                        if let Some(ct) = self.connection_type {
                            if provider.valid_for(ct) {
                                self.step = Step::InputBaseUrl;
                                continue;
                            }
                        }
                        self.provider = None;
                    }
                    self.list_state = ListState::default().with_selected(Some(0));
                    return Vec::new();
                }
                Step::InputBaseUrl => {
                    let provider = self.provider.unwrap();
                    if self.base_url.is_some() {
                        self.step = Step::InputApiKey;
                        continue;
                    }
                    if provider.is_local() && provider.default_base_url().is_none() {
                        self.step = Step::InputApiKey;
                        continue;
                    }
                    self.input = String::new();
                    self.cursor_pos = 0;
                    self.input_default = provider.default_base_url().map(|s| s.to_string());
                    self.input_label = "Base URL";
                    self.input_masked = false;
                    return Vec::new();
                }
                Step::InputApiKey => {
                    let provider = self.provider.unwrap();
                    if self.api_key.is_some() || provider.is_local() {
                        self.step = Step::Done;
                        continue;
                    }
                    self.input = String::new();
                    self.cursor_pos = 0;
                    self.input_default = None;
                    self.input_label = "API Key";
                    self.input_masked = true;
                    return Vec::new();
                }
                Step::Done => {
                    return vec![Effect::Save(SaveData {
                        connection_type: self.connection_type.unwrap(),
                        provider: self.provider.unwrap(),
                        base_url: self.base_url.clone(),
                        api_key: self.api_key.clone(),
                    })];
                }
            }
        }
    }
}

pub(crate) fn validate_base_url(input: &str) -> Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    Url::parse(trimmed)
        .map(|_| ())
        .map_err(|e| format!("invalid URL: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_args_provided_produces_save() {
        let (app, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Deepgram),
            Some("https://api.deepgram.com/v1".to_string()),
            Some("key123".to_string()),
            None,
            None,
        );
        assert_eq!(app.step(), Step::Done);
        assert!(matches!(effects.as_slice(), [Effect::Save(_)]));
    }

    #[test]
    fn no_args_starts_at_select_type() {
        let (app, effects) = App::new(None, None, None, None, None, None);
        assert_eq!(app.step(), Step::SelectType);
        assert!(effects.is_empty());
    }

    #[test]
    fn type_provided_starts_at_select_provider() {
        let (app, effects) = App::new(Some(ConnectionType::Stt), None, None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert!(effects.is_empty());
    }

    #[test]
    fn local_provider_skips_api_key() {
        let (app, effects) = App::new(
            Some(ConnectionType::Llm),
            Some(ConnectProvider::Ollama),
            None,
            None,
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputBaseUrl);
        assert!(effects.is_empty());
    }

    #[test]
    fn invalid_provider_for_type_clears_provider() {
        let (app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Anthropic),
            None,
            None,
            None,
            None,
        );
        assert_eq!(app.step(), Step::SelectProvider);
        assert!(app.provider().is_none());
    }

    #[test]
    fn select_type_then_provider() {
        let (mut app, _) = App::new(None, None, None, None, None, None);
        assert_eq!(app.step(), Step::SelectType);
        assert_eq!(app.list_state_mut().selected(), Some(0));

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(app.step(), Step::SelectProvider);
        assert_eq!(app.list_state_mut().selected(), Some(0));

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(app.step(), Step::InputBaseUrl);
    }

    #[test]
    fn select_provider_with_type_preset() {
        let (mut app, _) = App::new(Some(ConnectionType::Stt), None, None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert_eq!(app.list_state_mut().selected(), Some(0));

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(app.step(), Step::InputBaseUrl);
    }

    #[test]
    fn base_url_validation_rejects_invalid() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputBaseUrl);

        for c in "not-a-url".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert!(app.error().is_some());
    }

    #[test]
    fn esc_exits() {
        let (mut app, _) = App::new(None, None, None, None, None, None);
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Esc)));
        assert!(matches!(effects.as_slice(), [Effect::Exit]));
    }
}
