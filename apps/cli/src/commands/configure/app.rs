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
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Stt, Tab::Llm, Tab::Calendar];

    pub fn index(self) -> usize {
        match self {
            Tab::Stt => 0,
            Tab::Llm => 1,
            Tab::Calendar => 2,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Stt => "STT",
            Tab::Llm => "LLM",
            Tab::Calendar => "Calendar",
        }
    }
}

pub struct App {
    pub tab: Tab,
    pub current_stt: Option<String>,
    pub current_llm: Option<String>,
    pub stt_providers: Vec<String>,
    pub llm_providers: Vec<String>,
    pub calendars: Vec<CalendarRow>,
    pub provider_list_state: ListState,
    pub cal_cursor: usize,
    pub cal_permission: Option<CalendarPermissionState>,
    pub loading: bool,
    pub error: Option<String>,
}

impl App {
    pub fn new(initial_tab: Option<Tab>) -> (Self, Vec<Effect>) {
        let app = Self {
            tab: initial_tab.unwrap_or(Tab::Stt),
            current_stt: None,
            current_llm: None,
            stt_providers: Vec::new(),
            llm_providers: Vec::new(),
            calendars: Vec::new(),
            provider_list_state: ListState::default(),
            cal_cursor: 0,
            cal_permission: None,
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

    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Runtime(event) => self.handle_runtime(event),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return vec![Effect::Exit],
            KeyCode::Right | KeyCode::Tab => {
                self.switch_tab(1);
                return vec![];
            }
            KeyCode::Left | KeyCode::BackTab => {
                self.switch_tab(-1);
                return vec![];
            }
            _ => {}
        }

        match self.tab {
            Tab::Stt => self.handle_provider_key(key, true),
            Tab::Llm => self.handle_provider_key(key, false),
            Tab::Calendar => self.handle_calendar_key(key),
        }
    }

    fn handle_provider_key(&mut self, key: KeyEvent, is_stt: bool) -> Vec<Effect> {
        let count = if is_stt {
            self.stt_providers.len()
        } else {
            self.llm_providers.len()
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.provider_list_state.selected().unwrap_or(0);
                self.provider_list_state.select(Some(i.saturating_sub(1)));
                vec![]
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.provider_list_state.selected().unwrap_or(0);
                self.provider_list_state
                    .select(Some((i + 1).min(count.saturating_sub(1))));
                vec![]
            }
            KeyCode::Enter => {
                let idx = self.provider_list_state.selected().unwrap_or(0);
                let providers = if is_stt {
                    &self.stt_providers
                } else {
                    &self.llm_providers
                };
                if let Some(provider) = providers.get(idx) {
                    let provider = provider.clone();
                    if is_stt {
                        self.current_stt = Some(provider.clone());
                        vec![Effect::SaveSttProvider(provider)]
                    } else {
                        self.current_llm = Some(provider.clone());
                        vec![Effect::SaveLlmProvider(provider)]
                    }
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let authorized = self.cal_permission == Some(CalendarPermissionState::Authorized);

        if !authorized {
            match key.code {
                KeyCode::Enter => match self.cal_permission {
                    Some(CalendarPermissionState::NotDetermined) => {
                        vec![Effect::RequestCalendarPermission]
                    }
                    Some(CalendarPermissionState::Denied) => {
                        vec![Effect::ResetCalendarPermission]
                    }
                    _ => vec![],
                },
                _ => vec![],
            }
        } else {
            let item_count = self.calendars.len();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if item_count > 0 {
                        self.cal_cursor = self.cal_cursor.saturating_sub(1);
                    }
                    vec![]
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if item_count > 0 {
                        self.cal_cursor = (self.cal_cursor + 1).min(item_count.saturating_sub(1));
                    }
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
    }

    fn handle_runtime(&mut self, event: RuntimeEvent) -> Vec<Effect> {
        match event {
            RuntimeEvent::SettingsLoaded {
                current_stt,
                current_llm,
                stt_providers,
                llm_providers,
            } => {
                self.current_stt = current_stt;
                self.current_llm = current_llm;
                self.stt_providers = stt_providers;
                self.llm_providers = llm_providers;
                self.loading = false;
                self.reset_tab_state();
                vec![]
            }
            RuntimeEvent::CalendarsLoaded(calendars) => {
                self.calendars = calendars;
                vec![]
            }
            RuntimeEvent::CalendarPermissionStatus(state) => {
                self.cal_permission = Some(state);
                vec![]
            }
            RuntimeEvent::CalendarPermissionResult(granted) => {
                self.cal_permission = Some(if granted {
                    CalendarPermissionState::Authorized
                } else {
                    CalendarPermissionState::Denied
                });
                if granted {
                    vec![Effect::LoadCalendars]
                } else {
                    vec![]
                }
            }
            RuntimeEvent::CalendarPermissionReset => {
                self.cal_permission = None;
                vec![Effect::CheckCalendarPermission]
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
            Tab::Stt => {
                let idx = self
                    .current_stt
                    .as_ref()
                    .and_then(|c| self.stt_providers.iter().position(|p| p == c))
                    .unwrap_or(0);
                self.provider_list_state = ListState::default();
                self.provider_list_state.select(Some(idx));
            }
            Tab::Llm => {
                let idx = self
                    .current_llm
                    .as_ref()
                    .and_then(|c| self.llm_providers.iter().position(|p| p == c))
                    .unwrap_or(0);
                self.provider_list_state = ListState::default();
                self.provider_list_state.select(Some(idx));
            }
            Tab::Calendar => {
                self.cal_cursor = 0;
            }
        }
    }
}
