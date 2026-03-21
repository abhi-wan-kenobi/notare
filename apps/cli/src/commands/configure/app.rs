use crossterm::event::{KeyCode, KeyEvent};
use hypr_db_app::CalendarRow;
use ratatui::widgets::ListState;

use super::action::Action;
use super::effect::Effect;
use super::runtime::{CalendarPermissionState, RuntimeEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Stt,
    Llm,
    Calendar,
    Language,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Stt, Tab::Llm, Tab::Calendar, Tab::Language];

    pub fn index(self) -> usize {
        match self {
            Tab::Stt => 0,
            Tab::Llm => 1,
            Tab::Calendar => 2,
            Tab::Language => 3,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Stt => "STT",
            Tab::Llm => "LLM",
            Tab::Calendar => "Calendar",
            Tab::Language => "Language",
        }
    }

    pub fn setting_key(self) -> &'static str {
        match self {
            Tab::Stt => "current_stt_provider",
            Tab::Llm => "current_llm_provider",
            Tab::Calendar | Tab::Language => unreachable!(),
        }
    }

    pub fn connection_type(self) -> &'static str {
        match self {
            Tab::Stt => "stt",
            Tab::Llm => "llm",
            Tab::Calendar => "cal",
            Tab::Language => unreachable!(),
        }
    }
}

pub struct ProviderTab {
    pub current: Option<String>,
    pub providers: Vec<String>,
    pub list_state: ListState,
}

impl ProviderTab {
    fn new() -> Self {
        Self {
            current: None,
            providers: Vec::new(),
            list_state: ListState::default(),
        }
    }

    fn reset_cursor(&mut self) {
        let idx = self
            .current
            .as_ref()
            .and_then(|c| self.providers.iter().position(|p| p == c))
            .unwrap_or(0);
        self.list_state = ListState::default();
        self.list_state.select(Some(idx));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LanguageFocus {
    AiLanguage,
    SpokenLanguages,
}

pub struct LanguageTab {
    pub focus: LanguageFocus,
    pub languages: Vec<(String, String)>,
    pub ai_language: Option<String>,
    pub ai_list_state: ListState,
    pub spoken_languages: Vec<String>,
    pub spoken_cursor: usize,
}

impl LanguageTab {
    fn new() -> Self {
        let mut languages: Vec<(String, String)> = hypr_language::whisper_multilingual()
            .into_iter()
            .map(|l| {
                let code = l.iso639_code().to_string();
                let name = l.iso639().language_name().to_string();
                (code, name)
            })
            .collect();
        languages.sort_by(|a, b| a.1.cmp(&b.1));

        Self {
            focus: LanguageFocus::AiLanguage,
            languages,
            ai_language: None,
            ai_list_state: ListState::default(),
            spoken_languages: Vec::new(),
            spoken_cursor: 0,
        }
    }

    fn reset_cursor(&mut self) {
        let idx = self
            .ai_language
            .as_ref()
            .and_then(|c| self.languages.iter().position(|(code, _)| code == c))
            .unwrap_or(0);
        self.ai_list_state = ListState::default();
        self.ai_list_state.select(Some(idx));
        self.spoken_cursor = 0;
        self.focus = LanguageFocus::AiLanguage;
    }
}

pub struct App {
    pub tab: Tab,
    pub stt: ProviderTab,
    pub llm: ProviderTab,
    pub calendars: Vec<CalendarRow>,
    pub cal_cursor: usize,
    pub cal_permission: Option<CalendarPermissionState>,
    pub language: LanguageTab,
    pub loading: bool,
    pub error: Option<String>,
}

impl App {
    pub fn new(initial_tab: Option<Tab>) -> (Self, Vec<Effect>) {
        let app = Self {
            tab: initial_tab.unwrap_or(Tab::Stt),
            stt: ProviderTab::new(),
            llm: ProviderTab::new(),
            calendars: Vec::new(),
            cal_cursor: 0,
            cal_permission: None,
            language: LanguageTab::new(),
            loading: true,
            error: None,
        };

        (
            app,
            vec![
                Effect::LoadSettings,
                Effect::LoadCalendars,
                Effect::CheckCalendarPermission,
            ],
        )
    }

    fn provider_tab(&mut self) -> Option<&mut ProviderTab> {
        match self.tab {
            Tab::Stt => Some(&mut self.stt),
            Tab::Llm => Some(&mut self.llm),
            Tab::Calendar | Tab::Language => None,
        }
    }

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Runtime(event) => self.handle_runtime(event),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return vec![Effect::Exit],
            KeyCode::Right => {
                self.switch_tab(1);
                return vec![];
            }
            KeyCode::Left => {
                self.switch_tab(-1);
                return vec![];
            }
            KeyCode::Tab | KeyCode::BackTab if self.tab != Tab::Language => {
                let delta = if key.code == KeyCode::Tab { 1 } else { -1 };
                self.switch_tab(delta);
                return vec![];
            }
            _ => {}
        }

        match self.tab {
            Tab::Stt | Tab::Llm => self.handle_provider_key(key),
            Tab::Calendar => self.handle_calendar_key(key),
            Tab::Language => self.handle_language_key(key),
        }
    }

    fn handle_provider_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let tab = self.tab;
        let pt = self.provider_tab().unwrap();
        let count = pt.providers.len();

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = pt.list_state.selected().unwrap_or(0);
                pt.list_state.select(Some(i.saturating_sub(1)));
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = pt.list_state.selected().unwrap_or(0);
                pt.list_state
                    .select(Some((i + 1).min(count.saturating_sub(1))));
                vec![]
            }
            KeyCode::Enter => {
                let idx = pt.list_state.selected().unwrap_or(0);
                if let Some(provider) = pt.providers.get(idx).cloned() {
                    pt.current = Some(provider.clone());
                    vec![Effect::SaveProvider { tab, provider }]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let authorized = self.cal_permission == Some(CalendarPermissionState::Authorized);
        if !authorized || self.calendars.is_empty() {
            return vec![];
        }

        let item_count = self.calendars.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cal_cursor = self.cal_cursor.saturating_sub(1);
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cal_cursor = (self.cal_cursor + 1).min(item_count.saturating_sub(1));
                vec![]
            }
            KeyCode::Char(' ') => {
                if let Some(cal) = self.calendars.get_mut(self.cal_cursor) {
                    cal.enabled = !cal.enabled;
                }
                vec![]
            }
            KeyCode::Enter => {
                let calendars = self.calendars.clone();
                vec![Effect::SaveCalendars(calendars)]
            }
            _ => vec![],
        }
    }

    fn handle_runtime(&mut self, event: RuntimeEvent) -> Vec<Effect> {
        match event {
            RuntimeEvent::SettingsLoaded {
                current_stt,
                current_llm,
                stt_providers,
                llm_providers,
                ai_language,
                spoken_languages,
            } => {
                self.stt.current = current_stt;
                self.stt.providers = stt_providers;
                self.llm.current = current_llm;
                self.llm.providers = llm_providers;
                self.language.ai_language = ai_language;
                self.language.spoken_languages = spoken_languages;
                self.loading = false;
                self.reset_tab_state();
                vec![]
            }
            RuntimeEvent::CalendarsLoaded(mut calendars) => {
                calendars.sort_by(|a, b| a.source.cmp(&b.source));
                self.calendars = calendars;
                vec![]
            }
            RuntimeEvent::CalendarPermissionStatus(state) => {
                self.cal_permission = Some(state);
                vec![]
            }
            RuntimeEvent::Saved => {
                vec![]
            }
            RuntimeEvent::Error(msg) => {
                self.error = Some(msg);
                vec![]
            }
        }
    }

    fn switch_tab(&mut self, delta: i32) {
        let current = self.tab.index() as i32;
        let count = Tab::ALL.len() as i32;
        let next = (current + delta).rem_euclid(count) as usize;
        self.tab = Tab::ALL[next];
        self.reset_tab_state();
    }

    fn reset_tab_state(&mut self) {
        match self.tab {
            Tab::Stt => self.stt.reset_cursor(),
            Tab::Llm => self.llm.reset_cursor(),
            Tab::Calendar => {
                self.cal_cursor = 0;
            }
            Tab::Language => self.language.reset_cursor(),
        }
    }

    fn handle_language_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let lt = &mut self.language;
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                lt.focus = match lt.focus {
                    LanguageFocus::AiLanguage => LanguageFocus::SpokenLanguages,
                    LanguageFocus::SpokenLanguages => LanguageFocus::AiLanguage,
                };
                vec![]
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match lt.focus {
                    LanguageFocus::AiLanguage => {
                        let i = lt.ai_list_state.selected().unwrap_or(0);
                        lt.ai_list_state.select(Some(i.saturating_sub(1)));
                    }
                    LanguageFocus::SpokenLanguages => {
                        lt.spoken_cursor = lt.spoken_cursor.saturating_sub(1);
                    }
                }
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = lt.languages.len().saturating_sub(1);
                match lt.focus {
                    LanguageFocus::AiLanguage => {
                        let i = lt.ai_list_state.selected().unwrap_or(0);
                        lt.ai_list_state.select(Some((i + 1).min(max)));
                    }
                    LanguageFocus::SpokenLanguages => {
                        lt.spoken_cursor = (lt.spoken_cursor + 1).min(max);
                    }
                }
                vec![]
            }
            KeyCode::Enter => match lt.focus {
                LanguageFocus::AiLanguage => {
                    let idx = lt.ai_list_state.selected().unwrap_or(0);
                    if let Some((code, _)) = lt.languages.get(idx) {
                        lt.ai_language = Some(code.clone());
                        vec![Effect::SaveLanguage {
                            key: "ai_language".to_string(),
                            value: code.clone(),
                        }]
                    } else {
                        vec![]
                    }
                }
                LanguageFocus::SpokenLanguages => {
                    let json = serde_json::to_string(&lt.spoken_languages)
                        .unwrap_or_else(|_| "[]".to_string());
                    vec![Effect::SaveLanguage {
                        key: "spoken_languages".to_string(),
                        value: json,
                    }]
                }
            },
            KeyCode::Char(' ') => {
                if matches!(lt.focus, LanguageFocus::SpokenLanguages) {
                    let idx = lt.spoken_cursor;
                    if let Some((code, _)) = lt.languages.get(idx) {
                        if lt.spoken_languages.contains(code) {
                            lt.spoken_languages.retain(|c| c != code);
                        } else {
                            lt.spoken_languages.push(code.clone());
                        }
                    }
                }
                vec![]
            }
            _ => vec![],
        }
    }
}
