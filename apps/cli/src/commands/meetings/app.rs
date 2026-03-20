use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_db_app::{EventRow, MeetingRow};
use ratatui::widgets::ListState;

pub(crate) enum Outcome {
    Continue,
    Select(String),
    Exit,
}

pub(crate) struct App {
    events: Vec<EventRow>,
    meetings: Vec<MeetingRow>,
    calendar_configured: Option<bool>,
    list_state: ListState,
    meetings_loaded: bool,
    events_loaded: bool,
    error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            meetings: Vec::new(),
            calendar_configured: None,
            list_state: ListState::default(),
            meetings_loaded: false,
            events_loaded: false,
            error: None,
        }
    }

    pub fn set_meetings(&mut self, meetings: Vec<MeetingRow>) {
        self.meetings_loaded = true;
        self.meetings = meetings;
        if !self.meetings.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_events(&mut self, events: Vec<EventRow>) {
        self.events_loaded = true;
        self.calendar_configured = Some(true);
        self.events = events;
    }

    pub fn set_calendar_not_configured(&mut self) {
        self.events_loaded = true;
        self.calendar_configured = Some(false);
    }

    pub fn set_error(&mut self, msg: String) {
        self.meetings_loaded = true;
        self.events_loaded = true;
        self.error = Some(msg);
    }

    pub fn events(&self) -> &[EventRow] {
        &self.events
    }

    pub fn meetings(&self) -> &[MeetingRow] {
        &self.meetings
    }

    pub fn calendar_configured(&self) -> Option<bool> {
        self.calendar_configured
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn loading(&self) -> bool {
        !self.meetings_loaded || !self.events_loaded
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return Outcome::Exit;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.list_state.select_previous();
                Outcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.list_state.select_next();
                Outcome::Continue
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(meeting) = self.meetings.get(idx) {
                        return Outcome::Select(meeting.id.clone());
                    }
                }
                Outcome::Continue
            }
            KeyCode::Char('q') => Outcome::Exit,
            _ => Outcome::Continue,
        }
    }
}
