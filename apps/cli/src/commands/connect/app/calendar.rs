use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;

use crate::cli::ConnectProvider;

use super::super::effect::{CalendarSaveData, Effect};
use super::super::runtime::{CalendarItem, CalendarPermissionState, RuntimeEvent};

pub(crate) enum CalendarOutcome {
    Effects(Vec<Effect>),
    AdvanceToSelect(Vec<Effect>),
    Done(Vec<Effect>),
}

pub(crate) struct CalendarState {
    auth_status: Option<CalendarPermissionState>,
    loading: bool,
    items: Vec<CalendarItem>,
    enabled: Vec<bool>,
    list_state: ListState,
    error: Option<String>,
}

impl CalendarState {
    pub(crate) fn new() -> Self {
        Self {
            auth_status: None,
            loading: false,
            items: Vec::new(),
            enabled: Vec::new(),
            list_state: ListState::default(),
            error: None,
        }
    }

    pub(crate) fn auth_status(&self) -> Option<CalendarPermissionState> {
        self.auth_status
    }

    pub(crate) fn loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn items(&self) -> &[CalendarItem] {
        &self.items
    }

    pub(crate) fn enabled(&self) -> &[bool] {
        &self.enabled
    }

    pub(crate) fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn handle_permission_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.code != KeyCode::Enter {
            return Vec::new();
        }

        match self.auth_status {
            Some(CalendarPermissionState::NotDetermined) => {
                vec![Effect::RequestCalendarPermission]
            }
            Some(CalendarPermissionState::Denied) => {
                vec![Effect::ResetCalendarPermission]
            }
            Some(CalendarPermissionState::Authorized) => {
                self.error = None;
                self.loading = true;
                vec![Effect::LoadCalendars]
            }
            None => Vec::new(),
        }
    }

    pub(crate) fn handle_select_key(
        &mut self,
        key: KeyEvent,
        provider: ConnectProvider,
    ) -> Vec<Effect> {
        if self.loading {
            return Vec::new();
        }

        let len = self.items.len();
        if len == 0 {
            return Vec::new();
        }

        match key.code {
            KeyCode::Up => {
                let current = self.list_state.selected().unwrap_or(0);
                if current > 0 {
                    self.list_state.select(Some(current - 1));
                }
                Vec::new()
            }
            KeyCode::Down => {
                let current = self.list_state.selected().unwrap_or(0);
                if current + 1 < len {
                    self.list_state.select(Some(current + 1));
                }
                Vec::new()
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.list_state.selected() {
                    if idx < self.enabled.len() {
                        self.enabled[idx] = !self.enabled[idx];
                    }
                }
                Vec::new()
            }
            KeyCode::Enter => {
                let items: Vec<(CalendarItem, bool)> = self
                    .items
                    .iter()
                    .zip(self.enabled.iter())
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

    pub(crate) fn handle_runtime_event(&mut self, event: RuntimeEvent) -> CalendarOutcome {
        match event {
            RuntimeEvent::CalendarPermissionStatus(status) => {
                self.auth_status = Some(status);
                if status == CalendarPermissionState::Authorized {
                    self.error = None;
                    self.loading = true;
                    CalendarOutcome::AdvanceToSelect(vec![Effect::LoadCalendars])
                } else {
                    CalendarOutcome::Effects(Vec::new())
                }
            }
            RuntimeEvent::CalendarPermissionResult(granted) => {
                if granted {
                    self.auth_status = Some(CalendarPermissionState::Authorized);
                    self.error = None;
                    self.loading = true;
                    CalendarOutcome::AdvanceToSelect(vec![Effect::LoadCalendars])
                } else {
                    self.auth_status = Some(CalendarPermissionState::Denied);
                    CalendarOutcome::Effects(Vec::new())
                }
            }
            RuntimeEvent::CalendarPermissionReset => {
                self.auth_status = None;
                CalendarOutcome::Effects(vec![Effect::CheckCalendarPermission])
            }
            RuntimeEvent::CalendarsLoaded(mut items) => {
                self.error = None;
                items.sort_by(|a, b| a.source.cmp(&b.source));
                self.enabled = vec![true; items.len()];
                self.items = items;
                self.loading = false;
                if !self.items.is_empty() {
                    self.list_state.select(Some(0));
                }
                CalendarOutcome::Effects(Vec::new())
            }
            RuntimeEvent::CalendarsSaved => CalendarOutcome::Done(Vec::new()),
            RuntimeEvent::Error(msg) => {
                self.error = Some(msg);
                self.loading = false;
                CalendarOutcome::Effects(Vec::new())
            }
        }
    }
}
