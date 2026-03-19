use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use url::Url;

use crate::cli::{ConnectProvider, ConnectionType};

use super::action::Action;
use super::effect::{Effect, SaveData};
use super::providers::ALL_PROVIDERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    SelectProvider,
    InputBaseUrl,
    InputApiKey,
    Done,
}

pub(crate) struct App {
    step: Step,
    type_filter: Option<ConnectionType>,
    provider: Option<ConnectProvider>,
    base_url: Option<String>,
    api_key: Option<String>,
    list_state: ListState,
    search_query: String,
    input: String,
    cursor_pos: usize,
    input_default: Option<String>,
    input_label: &'static str,
    input_masked: bool,
    error: Option<String>,
    configured_providers: HashSet<String>,
}

impl App {
    pub fn new(
        type_filter: Option<ConnectionType>,
        provider: Option<ConnectProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> (Self, Vec<Effect>) {
        Self::new_with_configured(type_filter, provider, base_url, api_key, HashSet::new())
    }

    pub fn new_with_configured(
        type_filter: Option<ConnectionType>,
        provider: Option<ConnectProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
        configured_providers: HashSet<String>,
    ) -> (Self, Vec<Effect>) {
        let mut app = Self {
            step: Step::SelectProvider,
            type_filter,
            provider,
            base_url,
            api_key,
            list_state: ListState::default(),
            search_query: String::new(),
            input: String::new(),
            cursor_pos: 0,
            input_default: None,
            input_label: "",
            input_masked: false,
            error: None,
            configured_providers,
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

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn configured_providers(&self) -> &HashSet<String> {
        &self.configured_providers
    }

    pub fn filtered_providers(&self) -> Vec<ConnectProvider> {
        let query = self.search_query.to_ascii_lowercase();
        ALL_PROVIDERS
            .iter()
            .copied()
            .filter(|p| {
                if let Some(ct) = self.type_filter {
                    if !p.valid_for(ct) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                p.id().to_ascii_lowercase().contains(&query)
                    || p.display_name().to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn breadcrumb(&self) -> String {
        match self.provider {
            Some(p) => p.display_name().to_string(),
            None => String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return vec![Effect::Exit];
        }

        match self.step {
            Step::SelectProvider => self.handle_provider_key(key),
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
            Step::SelectProvider => {
                self.search_query.push_str(text);
                self.list_state.select(Some(0));
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_provider_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let filtered = self.filtered_providers();
        let len = filtered.len();

        match key.code {
            KeyCode::Up => {
                self.list_navigate(-1, len);
                Vec::new()
            }
            KeyCode::Down => {
                self.list_navigate(1, len);
                Vec::new()
            }
            KeyCode::Enter => {
                if len == 0 {
                    return Vec::new();
                }
                let idx = self.list_state.selected().unwrap_or(0);
                if let Some(&provider) = filtered.get(idx) {
                    if provider.is_disabled() {
                        return Vec::new();
                    }
                    self.provider = Some(provider);
                    self.step = Step::InputBaseUrl;
                    self.advance()
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.list_state.select(Some(0));
                Vec::new()
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.list_state.select(Some(0));
                Vec::new()
            }
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
                Step::SelectProvider => {
                    if self.provider.is_some() {
                        self.step = Step::InputBaseUrl;
                        continue;
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
                    if provider.default_base_url().is_some() {
                        self.base_url = provider.default_base_url().map(|s| s.to_string());
                        self.step = Step::InputApiKey;
                        continue;
                    }
                    if provider.is_local() {
                        self.step = Step::InputApiKey;
                        continue;
                    }
                    self.input = String::new();
                    self.cursor_pos = 0;
                    self.input_default = None;
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
                    let provider = self.provider.unwrap();
                    let mut connection_types = provider.capabilities();
                    if let Some(ct) = self.type_filter {
                        connection_types.retain(|t| *t == ct);
                    }
                    return vec![Effect::Save(SaveData {
                        connection_types,
                        provider,
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
    let parsed = Url::parse(trimmed).map_err(|e| format!("invalid URL: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("invalid URL: scheme must be http or https".to_string());
    }
    Ok(())
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
        );
        assert_eq!(app.step(), Step::Done);
        assert!(matches!(effects.as_slice(), [Effect::Save(_)]));
    }

    #[test]
    fn no_args_starts_at_select_provider() {
        let (app, effects) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert!(effects.is_empty());
    }

    #[test]
    fn provider_with_default_url_skips_base_url_input() {
        let (app, effects) = App::new(None, Some(ConnectProvider::Deepgram), None, None);
        assert_eq!(app.step(), Step::InputApiKey);
        assert!(effects.is_empty());
    }

    #[test]
    fn custom_provider_shows_base_url_input() {
        let (app, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputBaseUrl);
        assert!(effects.is_empty());
    }

    #[test]
    fn local_provider_skips_api_key() {
        let (app, effects) = App::new(None, Some(ConnectProvider::Ollama), None, None);
        assert_eq!(app.step(), Step::Done);
        assert!(matches!(effects.as_slice(), [Effect::Save(_)]));
    }

    #[test]
    fn search_filters_providers() {
        let (mut app, _) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('m'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('i'))));
        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char('s'))));

        let filtered = app.filtered_providers();
        assert!(filtered.contains(&ConnectProvider::Mistral));
        assert!(!filtered.contains(&ConnectProvider::Deepgram));
    }

    #[test]
    fn dual_capability_provider_produces_both_types() {
        let (_, effects) = App::new(
            None,
            Some(ConnectProvider::Openai),
            Some("https://api.openai.com/v1".to_string()),
            Some("key".to_string()),
        );
        if let Effect::Save(data) = &effects[0] {
            assert!(data.connection_types.contains(&ConnectionType::Stt));
            assert!(data.connection_types.contains(&ConnectionType::Llm));
        } else {
            panic!("expected Save effect");
        }
    }

    #[test]
    fn type_filter_restricts_connection_types() {
        let (_, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Openai),
            Some("https://api.openai.com/v1".to_string()),
            Some("key".to_string()),
        );
        if let Effect::Save(data) = &effects[0] {
            assert_eq!(data.connection_types, vec![ConnectionType::Stt]);
        } else {
            panic!("expected Save effect");
        }
    }

    #[test]
    fn select_provider_then_input() {
        let (mut app, _) = App::new(None, None, None, None);
        assert_eq!(app.step(), Step::SelectProvider);
        assert_eq!(app.list_state_mut().selected(), Some(0));

        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        // First provider (Deepgram) has a default URL, so it skips to InputApiKey
        assert_eq!(app.step(), Step::InputApiKey);
    }

    #[test]
    fn base_url_validation_rejects_invalid() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
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

        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputBaseUrl);

        for c in "ftp://example.com".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(
            app.error(),
            Some("invalid URL: scheme must be http or https")
        );
    }

    #[test]
    fn esc_exits() {
        let (mut app, _) = App::new(None, None, None, None);
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Esc)));
        assert!(matches!(effects.as_slice(), [Effect::Exit]));
    }
}
