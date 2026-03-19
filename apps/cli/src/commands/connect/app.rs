use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use url::Url;

use crate::cli::{ConnectProvider, ConnectionType};

use super::action::Action;
use super::effect::{CalendarSaveData, Effect, SaveData};
use super::providers::ALL_PROVIDERS;
use super::runtime::{CalendarItem, CalendarPermissionState, RuntimeEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    SelectProvider,
    InputForm,
    CalendarPermission,
    CalendarSelect,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormFieldId {
    BaseUrl,
    ApiKey,
}

pub(crate) struct FormField {
    pub id: FormFieldId,
    pub label: &'static str,
    pub value: String,
    pub cursor_pos: usize,
    pub default: Option<String>,
    pub masked: bool,
    pub error: Option<String>,
}

impl FormField {
    fn new(id: FormFieldId, label: &'static str, masked: bool, default: Option<String>) -> Self {
        Self {
            id,
            label,
            value: String::new(),
            cursor_pos: 0,
            default,
            masked,
            error: None,
        }
    }

    fn byte_index(&self) -> usize {
        self.value
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_pos)
            .unwrap_or(self.value.len())
    }

    fn effective_value(&self) -> Option<String> {
        if self.value.trim().is_empty() {
            self.default.clone()
        } else {
            Some(self.value.trim().to_string())
        }
    }
}

pub(crate) struct App {
    step: Step,
    type_filter: Option<ConnectionType>,
    provider: Option<ConnectProvider>,
    base_url: Option<String>,
    api_key: Option<String>,
    list_state: ListState,
    search_query: String,
    form_fields: Vec<FormField>,
    focused_field: usize,
    configured_providers: HashSet<String>,
    // Calendar state
    cal_auth_status: Option<CalendarPermissionState>,
    cal_loading: bool,
    cal_items: Vec<CalendarItem>,
    cal_enabled: Vec<bool>,
    cal_list_state: ListState,
    cal_error: Option<String>,
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
            form_fields: Vec::new(),
            focused_field: 0,
            configured_providers,
            cal_auth_status: None,
            cal_loading: false,
            cal_items: Vec::new(),
            cal_enabled: Vec::new(),
            cal_list_state: ListState::default(),
            cal_error: None,
        };
        let effects = app.advance();
        (app, effects)
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(text) => self.handle_paste(&text),
            Action::Runtime(event) => self.handle_runtime_event(event),
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn provider(&self) -> Option<ConnectProvider> {
        self.provider
    }

    pub fn form_fields(&self) -> &[FormField] {
        &self.form_fields
    }

    pub fn focused_field(&self) -> usize {
        self.focused_field
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

    pub fn cal_auth_status(&self) -> Option<CalendarPermissionState> {
        self.cal_auth_status
    }

    pub fn cal_loading(&self) -> bool {
        self.cal_loading
    }

    pub fn cal_items(&self) -> &[CalendarItem] {
        &self.cal_items
    }

    pub fn cal_enabled(&self) -> &[bool] {
        &self.cal_enabled
    }

    pub fn cal_list_state_mut(&mut self) -> &mut ListState {
        &mut self.cal_list_state
    }

    pub fn cal_error(&self) -> Option<&str> {
        self.cal_error.as_deref()
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
            Step::InputForm => self.handle_form_key(key),
            Step::CalendarPermission => self.handle_permission_key(key),
            Step::CalendarSelect => self.handle_calendar_select_key(key),
            Step::Done => Vec::new(),
        }
    }

    fn handle_paste(&mut self, text: &str) -> Vec<Effect> {
        match self.step {
            Step::InputForm => {
                let field = &mut self.form_fields[self.focused_field];
                for c in text.chars() {
                    let idx = field.byte_index();
                    field.value.insert(idx, c);
                    field.cursor_pos += 1;
                }
                field.error = None;
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
                    self.step = Step::InputForm;
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

    fn handle_form_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Tab => {
                if self.form_fields.len() > 1 {
                    self.focused_field = (self.focused_field + 1) % self.form_fields.len();
                }
                Vec::new()
            }
            KeyCode::BackTab => {
                if self.form_fields.len() > 1 {
                    self.focused_field = if self.focused_field == 0 {
                        self.form_fields.len() - 1
                    } else {
                        self.focused_field - 1
                    };
                }
                Vec::new()
            }
            KeyCode::Enter => {
                if self.confirm_form() {
                    self.step = Step::Done;
                    self.advance()
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char(c) => {
                let field = &mut self.form_fields[self.focused_field];
                let idx = field.byte_index();
                field.value.insert(idx, c);
                field.cursor_pos += 1;
                field.error = None;
                Vec::new()
            }
            KeyCode::Backspace => {
                let field = &mut self.form_fields[self.focused_field];
                if field.cursor_pos > 0 {
                    field.cursor_pos -= 1;
                    let idx = field.byte_index();
                    field.value.remove(idx);
                }
                field.error = None;
                Vec::new()
            }
            KeyCode::Left => {
                let field = &mut self.form_fields[self.focused_field];
                field.cursor_pos = field.cursor_pos.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Right => {
                let field = &mut self.form_fields[self.focused_field];
                let max = field.value.chars().count();
                if field.cursor_pos < max {
                    field.cursor_pos += 1;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_permission_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code != KeyCode::Enter {
            return Vec::new();
        }

        match self.cal_auth_status {
            Some(CalendarPermissionState::NotDetermined) => {
                vec![Effect::RequestCalendarPermission]
            }
            Some(CalendarPermissionState::Denied) => {
                vec![Effect::ResetCalendarPermission]
            }
            Some(CalendarPermissionState::Authorized) => {
                self.cal_error = None;
                self.cal_loading = true;
                self.step = Step::CalendarSelect;
                vec![Effect::LoadCalendars]
            }
            None => Vec::new(),
        }
    }

    fn handle_calendar_select_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if self.cal_loading {
            return Vec::new();
        }

        let len = self.cal_items.len();
        if len == 0 {
            return Vec::new();
        }

        match key.code {
            KeyCode::Up => {
                let current = self.cal_list_state.selected().unwrap_or(0);
                if current > 0 {
                    self.cal_list_state.select(Some(current - 1));
                }
                Vec::new()
            }
            KeyCode::Down => {
                let current = self.cal_list_state.selected().unwrap_or(0);
                if current + 1 < len {
                    self.cal_list_state.select(Some(current + 1));
                }
                Vec::new()
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.cal_list_state.selected() {
                    if idx < self.cal_enabled.len() {
                        self.cal_enabled[idx] = !self.cal_enabled[idx];
                    }
                }
                Vec::new()
            }
            KeyCode::Enter => {
                let provider = self.provider.unwrap();
                let items: Vec<(CalendarItem, bool)> = self
                    .cal_items
                    .iter()
                    .zip(self.cal_enabled.iter())
                    .map(|(item, &enabled)| (item.clone(), enabled))
                    .collect();
                vec![Effect::SaveCalendars(CalendarSaveData {
                    provider: provider.id().to_string(),
                    items,
                })]
            }
            _ => Vec::new(),
        }
    }

    fn handle_runtime_event(&mut self, event: RuntimeEvent) -> Vec<Effect> {
        match event {
            RuntimeEvent::CalendarPermissionStatus(status) => {
                self.cal_auth_status = Some(status);
                if status == CalendarPermissionState::Authorized
                    && self.step == Step::CalendarPermission
                {
                    self.cal_error = None;
                    self.cal_loading = true;
                    self.step = Step::CalendarSelect;
                    vec![Effect::LoadCalendars]
                } else {
                    Vec::new()
                }
            }
            RuntimeEvent::CalendarPermissionResult(granted) => {
                if granted {
                    self.cal_auth_status = Some(CalendarPermissionState::Authorized);
                    self.cal_error = None;
                    self.cal_loading = true;
                    self.step = Step::CalendarSelect;
                    vec![Effect::LoadCalendars]
                } else {
                    self.cal_auth_status = Some(CalendarPermissionState::Denied);
                    Vec::new()
                }
            }
            RuntimeEvent::CalendarPermissionReset => {
                self.cal_auth_status = None;
                vec![Effect::CheckCalendarPermission]
            }
            RuntimeEvent::CalendarsLoaded(mut items) => {
                self.cal_error = None;
                items.sort_by(|a, b| a.source.cmp(&b.source));
                self.cal_enabled = vec![true; items.len()];
                self.cal_items = items;
                self.cal_loading = false;
                if !self.cal_items.is_empty() {
                    self.cal_list_state.select(Some(0));
                }
                Vec::new()
            }
            RuntimeEvent::CalendarsSaved => {
                self.step = Step::Done;
                self.advance()
            }
            RuntimeEvent::Error(msg) => {
                self.cal_error = Some(msg);
                self.cal_loading = false;
                Vec::new()
            }
        }
    }

    fn confirm_form(&mut self) -> bool {
        let mut all_valid = true;

        for field in &mut self.form_fields {
            field.error = None;
            let value = field.effective_value();

            if field.id == FormFieldId::BaseUrl {
                if let Some(ref url) = value {
                    if let Err(msg) = validate_base_url(url) {
                        field.error = Some(msg);
                        all_valid = false;
                    }
                }
            }
        }

        if all_valid {
            for i in 0..self.form_fields.len() {
                let value = self.form_fields[i].effective_value();
                match self.form_fields[i].id {
                    FormFieldId::BaseUrl => self.base_url = value,
                    FormFieldId::ApiKey => self.api_key = value,
                }
            }
        }

        all_valid
    }

    fn advance(&mut self) -> Vec<Effect> {
        loop {
            match self.step {
                Step::SelectProvider => {
                    if self.provider.is_some() {
                        self.step = Step::InputForm;
                        continue;
                    }
                    self.list_state = ListState::default().with_selected(Some(0));
                    return Vec::new();
                }
                Step::InputForm => {
                    let provider = self.provider.unwrap();

                    if provider.is_local() && provider.is_calendar_provider() {
                        self.step = Step::CalendarPermission;
                        return vec![Effect::CheckCalendarPermission];
                    }

                    let mut fields = Vec::new();

                    if self.base_url.is_none() {
                        if let Some(default) = provider.default_base_url() {
                            self.base_url = Some(default.to_string());
                        } else if !provider.is_local() {
                            fields.push(FormField::new(
                                FormFieldId::BaseUrl,
                                "Base URL",
                                false,
                                None,
                            ));
                        }
                    }

                    if self.api_key.is_none() && !provider.is_local() {
                        fields.push(FormField::new(FormFieldId::ApiKey, "API Key", true, None));
                    }

                    if fields.is_empty() {
                        self.step = Step::Done;
                        continue;
                    }

                    self.form_fields = fields;
                    self.focused_field = 0;
                    return Vec::new();
                }
                Step::CalendarPermission | Step::CalendarSelect => {
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
    fn provider_with_default_url_shows_api_key_only() {
        let (app, effects) = App::new(None, Some(ConnectProvider::Deepgram), None, None);
        assert_eq!(app.step(), Step::InputForm);
        assert!(effects.is_empty());
        assert_eq!(app.form_fields().len(), 1);
        assert_eq!(app.form_fields()[0].id, FormFieldId::ApiKey);
    }

    #[test]
    fn custom_provider_shows_both_fields() {
        let (app, effects) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);
        assert!(effects.is_empty());
        assert_eq!(app.form_fields().len(), 2);
        assert_eq!(app.form_fields()[0].id, FormFieldId::BaseUrl);
        assert_eq!(app.form_fields()[1].id, FormFieldId::ApiKey);
    }

    #[test]
    fn local_provider_skips_form() {
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
        // First provider (Deepgram) has a default URL, so form shows only API key
        assert_eq!(app.step(), Step::InputForm);
        assert_eq!(app.form_fields().len(), 1);
        assert_eq!(app.form_fields()[0].id, FormFieldId::ApiKey);
    }

    #[test]
    fn base_url_validation_rejects_invalid() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);

        for c in "not-a-url".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert!(app.form_fields()[0].error.is_some());

        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);

        for c in "ftp://example.com".chars() {
            app.dispatch(Action::Key(KeyEvent::from(KeyCode::Char(c))));
        }
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Enter)));
        assert!(effects.is_empty());
        assert_eq!(
            app.form_fields()[0].error.as_deref(),
            Some("invalid URL: scheme must be http or https")
        );
    }

    #[test]
    fn esc_exits() {
        let (mut app, _) = App::new(None, None, None, None);
        let effects = app.dispatch(Action::Key(KeyEvent::from(KeyCode::Esc)));
        assert!(matches!(effects.as_slice(), [Effect::Exit]));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_calendar_goes_to_permission_step() {
        let (app, effects) = App::new(None, Some(ConnectProvider::AppleCalendar), None, None);
        assert_eq!(app.step(), Step::CalendarPermission);
        assert!(matches!(
            effects.as_slice(),
            [Effect::CheckCalendarPermission]
        ));
    }

    #[test]
    fn authorized_calendar_permission_auto_loads_calendars() {
        let (mut app, _) = App::new(None, Some(ConnectProvider::AppleCalendar), None, None);

        let effects = app.dispatch(Action::Runtime(RuntimeEvent::CalendarPermissionStatus(
            CalendarPermissionState::Authorized,
        )));

        assert_eq!(app.step(), Step::CalendarSelect);
        assert!(app.cal_loading());
        assert!(matches!(effects.as_slice(), [Effect::LoadCalendars]));
    }

    #[test]
    fn tab_cycles_form_fields() {
        let (mut app, _) = App::new(
            Some(ConnectionType::Stt),
            Some(ConnectProvider::Custom),
            None,
            None,
        );
        assert_eq!(app.step(), Step::InputForm);
        assert_eq!(app.form_fields().len(), 2);
        assert_eq!(app.focused_field(), 0);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(app.focused_field(), 1);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(app.focused_field(), 0);

        app.dispatch(Action::Key(KeyEvent::from(KeyCode::BackTab)));
        assert_eq!(app.focused_field(), 1);
    }
}
