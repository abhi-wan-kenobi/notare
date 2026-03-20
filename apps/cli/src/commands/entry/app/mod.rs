mod commands;
mod search;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::{Editor, KeyResult};
use ratatui_image::protocol::StatefulProtocol;

use crate::commands::connect;
use crate::commands::meetings;
use crate::commands::model;
use crate::commands::timeline;

pub(crate) use commands::{ALL_COMMANDS, Command, CommandEntry};

use commands::{load_logo_protocol, lookup, pick_tip};
use search::command_match_score;

use super::action::Action;
use super::effect::Effect;

enum MeetingsIntent {
    View,
    ChatResume,
}

pub(crate) enum Overlay {
    None,
    Connect(connect::app::App),
    Meetings(meetings::app::App),
    Models(model::app::App),
    Timeline(timeline::app::App),
}

pub(crate) struct App {
    input: Editor,
    filtered_commands: Vec<usize>,
    selected_index: usize,
    popup_visible: bool,
    pub(crate) status_message: Option<String>,
    pub(crate) tip: &'static str,
    logo_protocol: Option<StatefulProtocol>,
    pub(crate) stt_provider: Option<String>,
    pub(crate) llm_provider: Option<String>,
    overlay: Overlay,
    meetings_intent: MeetingsIntent,
}

impl App {
    pub(crate) fn new(
        status_message: Option<String>,
        stt_provider: Option<String>,
        llm_provider: Option<String>,
    ) -> Self {
        let mut app = Self {
            input: Editor::single_line(),
            filtered_commands: Vec::new(),
            selected_index: 0,
            popup_visible: false,
            status_message,
            tip: pick_tip(&stt_provider, &llm_provider),
            logo_protocol: load_logo_protocol(),
            stt_provider,
            llm_provider,
            overlay: Overlay::None,
            meetings_intent: MeetingsIntent::View,
        };
        app.recompute_popup();
        app
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        match action {
            Action::Key(key) => self.handle_key(key),
            Action::Paste(pasted) => self.handle_paste(pasted),
            Action::SubmitCommand(command) => self.submit_command(&command),
            Action::StatusMessage(message) => {
                self.status_message = Some(message);
                self.input = Editor::single_line();
                self.recompute_popup();
                Vec::new()
            }
        }
    }

    pub(crate) fn handle_connect_runtime(
        &mut self,
        event: connect::runtime::RuntimeEvent,
    ) -> Vec<Effect> {
        if let Overlay::Connect(ref mut app) = self.overlay {
            let effects = app.dispatch(connect::action::Action::Runtime(event));
            self.translate_connect_effects(effects)
        } else {
            Vec::new()
        }
    }

    pub(crate) fn handle_meetings_loaded(
        &mut self,
        meetings: Vec<hypr_db_app::MeetingRow>,
    ) -> Vec<Effect> {
        if let Overlay::Meetings(ref mut app) = self.overlay {
            app.set_meetings(meetings);
        }
        Vec::new()
    }

    pub(crate) fn handle_meetings_load_error(&mut self, msg: String) -> Vec<Effect> {
        if let Overlay::Meetings(ref mut app) = self.overlay {
            app.set_error(msg);
        }
        Vec::new()
    }

    pub(crate) fn handle_events_loaded(
        &mut self,
        events: Vec<hypr_db_app::EventRow>,
    ) -> Vec<Effect> {
        if let Overlay::Meetings(ref mut app) = self.overlay {
            app.set_events(events);
        }
        Vec::new()
    }

    pub(crate) fn handle_calendar_not_configured(&mut self) -> Vec<Effect> {
        if let Overlay::Meetings(ref mut app) = self.overlay {
            app.set_calendar_not_configured();
        }
        Vec::new()
    }

    pub(crate) fn handle_models_loaded(
        &mut self,
        models: Vec<crate::commands::model::list::ModelRow>,
    ) -> Vec<Effect> {
        if let Overlay::Models(ref mut app) = self.overlay {
            app.set_models(models);
        }
        Vec::new()
    }

    pub(crate) fn handle_connect_saved(
        &mut self,
        connection_types: Vec<crate::commands::connect::ConnectionType>,
        provider_id: String,
    ) -> Vec<Effect> {
        for ct in &connection_types {
            match ct {
                crate::commands::connect::ConnectionType::Stt => {
                    self.stt_provider = Some(provider_id.clone());
                }
                crate::commands::connect::ConnectionType::Llm => {
                    self.llm_provider = Some(provider_id.clone());
                }
                _ => {}
            }
        }
        self.tip = pick_tip(&self.stt_provider, &self.llm_provider);
        self.status_message = Some("Provider configured".into());
        Vec::new()
    }

    pub(crate) fn handle_timeline_contacts_loaded(
        &mut self,
        orgs: Vec<hypr_db_app::OrganizationRow>,
        humans: Vec<hypr_db_app::HumanRow>,
    ) -> Vec<Effect> {
        if let Overlay::Timeline(ref mut app) = self.overlay {
            app.set_contacts(orgs, humans);
        }
        Vec::new()
    }

    pub(crate) fn handle_timeline_contacts_load_error(&mut self, msg: String) -> Vec<Effect> {
        if let Overlay::Timeline(ref mut app) = self.overlay {
            app.set_error(msg);
        }
        Vec::new()
    }

    pub(crate) fn handle_timeline_entries_loaded(
        &mut self,
        entries: Vec<hypr_db_app::TimelineRow>,
    ) -> Vec<Effect> {
        if let Overlay::Timeline(ref mut app) = self.overlay {
            app.set_entries(entries);
        }
        Vec::new()
    }

    pub(crate) fn handle_timeline_entries_load_error(&mut self, msg: String) -> Vec<Effect> {
        if let Overlay::Timeline(ref mut app) = self.overlay {
            app.set_entries_error(msg);
        }
        Vec::new()
    }

    pub(crate) fn reload_logo(&mut self) {
        self.logo_protocol = load_logo_protocol();
    }

    pub(crate) fn logo_protocol(&mut self) -> Option<&mut StatefulProtocol> {
        self.logo_protocol.as_mut()
    }

    pub(crate) fn cursor_col(&self) -> usize {
        self.input.cursor().1
    }

    pub(crate) fn input_text(&self) -> String {
        self.input.lines().first().cloned().unwrap_or_default()
    }

    pub(crate) fn query(&self) -> String {
        self.input_text()
            .trim()
            .trim_start_matches('/')
            .to_ascii_lowercase()
    }

    pub(crate) fn overlay_mut(&mut self) -> &mut Overlay {
        &mut self.overlay
    }

    pub(crate) fn has_overlay(&self) -> bool {
        !matches!(self.overlay, Overlay::None)
    }

    pub(crate) fn popup_visible(&self) -> bool {
        self.popup_visible
    }

    pub(crate) fn visible_commands(&self) -> Vec<CommandEntry> {
        self.filtered_commands
            .iter()
            .filter_map(|&i| {
                let cmd = ALL_COMMANDS.get(i)?;
                Some(CommandEntry {
                    name: cmd.name(),
                    description: cmd.description(),
                    group: cmd.group(),
                    disabled_reason: cmd.disabled_reason(&self.stt_provider, &self.llm_provider),
                })
            })
            .collect()
    }

    fn is_command_disabled(&self, cmd: Command) -> bool {
        cmd.disabled_reason(&self.stt_provider, &self.llm_provider)
            .is_some()
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match self.overlay {
            Overlay::Connect(ref mut app) => {
                let effects = app.dispatch(connect::action::Action::Key(key));
                return self.translate_connect_effects(effects);
            }
            Overlay::Meetings(ref mut app) => {
                let outcome = app.handle_key(key);
                return self.handle_meetings_outcome(outcome);
            }
            Overlay::Models(ref mut app) => {
                if app.handle_key(key) {
                    self.reset_input();
                }
                return Vec::new();
            }
            Overlay::Timeline(ref mut app) => {
                let outcome = app.handle_key(key);
                return self.handle_timeline_outcome(outcome);
            }
            Overlay::None => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Exit];
        }

        if key.code == KeyCode::Esc {
            self.reset_input();
            return Vec::new();
        }

        if self.popup_visible {
            match key.code {
                KeyCode::Up => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    return Vec::new();
                }
                KeyCode::Down => {
                    let max = self.filtered_commands.len().saturating_sub(1);
                    self.selected_index = (self.selected_index + 1).min(max);
                    return Vec::new();
                }
                KeyCode::Tab => {
                    if let Some(cmd) = self.selected_command_name() {
                        self.set_input_text(cmd.to_string());
                        self.recompute_popup();
                    }
                    return Vec::new();
                }
                _ => {}
            }
        }

        if key.code == KeyCode::Enter {
            if self.popup_visible
                && let Some(cmd) = self.selected_command_name()
            {
                self.set_input_text(cmd.to_string());
            }

            let command = self.input_text().trim().to_string();
            return self.submit_command(&command);
        }

        if self.input.handle_key(key) == KeyResult::Consumed {
            self.status_message = None;
            self.recompute_popup();
        }

        Vec::new()
    }

    fn handle_paste(&mut self, pasted: String) -> Vec<Effect> {
        match self.overlay {
            Overlay::Connect(ref mut app) => {
                let effects = app.dispatch(connect::action::Action::Paste(pasted));
                return self.translate_connect_effects(effects);
            }
            Overlay::Meetings(_) => return Vec::new(),
            Overlay::Models(_) => return Vec::new(),
            Overlay::Timeline(_) => return Vec::new(),
            Overlay::None => {}
        }

        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        if !pasted.is_empty() {
            self.input.insert_str(&pasted);
            self.status_message = None;
            self.recompute_popup();
        }
        Vec::new()
    }

    fn submit_command(&mut self, command: &str) -> Vec<Effect> {
        let normalized = command.trim().trim_start_matches('/').to_ascii_lowercase();
        if normalized.is_empty() {
            return Vec::new();
        }

        let Some((cmd, rest)) = lookup(&normalized) else {
            self.status_message = Some(format!("Unknown command: {}", command.trim()));
            return Vec::new();
        };
        if self.is_command_disabled(cmd) {
            return Vec::new();
        }
        self.dispatch_command(cmd, rest)
    }

    fn dispatch_command(&mut self, cmd: Command, rest: &str) -> Vec<Effect> {
        use crate::commands::model::Commands as ModelCommands;

        match cmd {
            Command::MeetingsNew => vec![Effect::Launch(super::EntryCommand::MeetingsNew)],
            Command::Chat => vec![Effect::Launch(super::EntryCommand::Chat {
                session_id: None,
            })],
            Command::ChatResume => {
                self.reset_input();
                self.meetings_intent = MeetingsIntent::ChatResume;
                self.overlay = Overlay::Meetings(meetings::app::App::new());
                vec![Effect::LoadMeetings]
            }
            Command::Meetings => {
                self.reset_input();
                self.meetings_intent = MeetingsIntent::View;
                self.overlay = Overlay::Meetings(meetings::app::App::new());
                vec![Effect::LoadMeetings]
            }
            Command::Timeline => {
                self.reset_input();
                self.overlay = Overlay::Timeline(timeline::app::App::new());
                vec![Effect::LoadTimelineContacts]
            }
            Command::Connect => {
                let (connect_app, initial_effects) = connect::app::App::new(None, None, None, None);
                self.reset_input();
                self.overlay = Overlay::Connect(connect_app);
                self.translate_connect_effects(initial_effects)
            }
            Command::Auth => {
                self.reset_input();
                vec![Effect::OpenAuth]
            }
            Command::Bug => {
                self.reset_input();
                vec![Effect::OpenBug]
            }
            Command::Hello => {
                self.reset_input();
                vec![Effect::OpenHello]
            }
            Command::Desktop => {
                self.reset_input();
                vec![Effect::OpenDesktop]
            }
            Command::Models => {
                self.reset_input();
                self.overlay = Overlay::Models(model::app::App::new());
                vec![Effect::LoadModels]
            }
            Command::ModelsDownload => {
                let name = rest.to_string();
                if name.is_empty() {
                    self.reset_input();
                    self.status_message = Some("Usage: /models download <name>".to_string());
                    Vec::new()
                } else {
                    vec![Effect::RunModel(ModelCommands::Download { name })]
                }
            }
            Command::ModelsDelete => {
                let name = rest.to_string();
                if name.is_empty() {
                    self.reset_input();
                    self.status_message = Some("Usage: /models delete <name>".to_string());
                    Vec::new()
                } else {
                    vec![Effect::RunModel(ModelCommands::Delete { name })]
                }
            }
            Command::ModelsPaths => vec![Effect::RunModel(ModelCommands::Paths)],
            Command::Exit => vec![Effect::Exit],
        }
    }

    fn translate_connect_effects(&mut self, effects: Vec<connect::effect::Effect>) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                connect::effect::Effect::Save(data) => {
                    self.reset_input();
                    result.push(Effect::SaveConnect {
                        connection_types: data.connection_types,
                        provider: data.provider,
                        base_url: data.base_url,
                        api_key: data.api_key,
                    });
                }
                connect::effect::Effect::Exit => {
                    self.reset_input();
                }
                connect::effect::Effect::CheckCalendarPermission => {
                    result.push(Effect::CheckCalendarPermission);
                }
                connect::effect::Effect::RequestCalendarPermission => {
                    result.push(Effect::RequestCalendarPermission);
                }
                connect::effect::Effect::ResetCalendarPermission => {
                    result.push(Effect::ResetCalendarPermission);
                }
                connect::effect::Effect::LoadCalendars => {
                    result.push(Effect::LoadCalendars);
                }
                connect::effect::Effect::SaveCalendars(data) => {
                    result.push(Effect::SaveCalendars(data));
                }
            }
        }
        result
    }

    fn handle_meetings_outcome(&mut self, outcome: meetings::app::Outcome) -> Vec<Effect> {
        match outcome {
            meetings::app::Outcome::Continue => Vec::new(),
            meetings::app::Outcome::Select(id) => {
                let cmd = match self.meetings_intent {
                    MeetingsIntent::View => super::EntryCommand::View { session_id: id },
                    MeetingsIntent::ChatResume => super::EntryCommand::Chat {
                        session_id: Some(id),
                    },
                };
                self.reset_input();
                vec![Effect::Launch(cmd)]
            }
            meetings::app::Outcome::Exit => {
                self.reset_input();
                Vec::new()
            }
        }
    }

    fn handle_timeline_outcome(&mut self, outcome: timeline::app::Outcome) -> Vec<Effect> {
        match outcome {
            timeline::app::Outcome::Continue => Vec::new(),
            timeline::app::Outcome::LoadTimeline(human_id) => {
                vec![Effect::LoadTimelineEntries(human_id)]
            }
            timeline::app::Outcome::ViewSession(session_id) => {
                self.reset_input();
                vec![Effect::Launch(super::EntryCommand::View { session_id })]
            }
            timeline::app::Outcome::Exit => {
                self.reset_input();
                Vec::new()
            }
        }
    }

    fn reset_input(&mut self) {
        let had_overlay = self.has_overlay();
        self.overlay = Overlay::None;
        self.input = Editor::single_line();
        self.status_message = None;
        self.recompute_popup();
        if had_overlay {
            self.reload_logo();
        }
    }

    fn selected_command_name(&self) -> Option<&'static str> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        Some(ALL_COMMANDS.get(selected)?.name())
    }

    fn set_input_text(&mut self, value: String) {
        self.input = Editor::single_line();
        self.input.insert_str(&value);
    }

    fn recompute_popup(&mut self) {
        let input = self.input_text();
        let input = input.trim();

        if input.is_empty() {
            self.popup_visible = false;
            self.filtered_commands.clear();
            self.selected_index = 0;
            return;
        }

        self.popup_visible = true;
        let query = input.trim_start_matches('/');
        let mut ranked = ALL_COMMANDS
            .iter()
            .enumerate()
            .filter_map(|(i, command)| {
                command_match_score(query, command.name()).map(|score| (i, score))
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|(left_i, left_score), (right_i, right_score)| {
            right_score.cmp(left_score).then_with(|| {
                ALL_COMMANDS[*left_i]
                    .name()
                    .cmp(ALL_COMMANDS[*right_i].name())
            })
        });

        self.filtered_commands = ranked.into_iter().map(|(i, _)| i).collect();

        use super::ui::command_popup::GROUP_ORDER;
        self.filtered_commands.sort_by_key(|&i| {
            let group = ALL_COMMANDS[i].group();
            GROUP_ORDER
                .iter()
                .position(|&g| g == group)
                .unwrap_or(usize::MAX)
        });

        if self.filtered_commands.is_empty() {
            self.filtered_commands = (0..ALL_COMMANDS.len()).collect();
        }

        self.selected_index = self
            .selected_index
            .min(self.filtered_commands.len().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_runtime_events_update_connect_overlay() {
        let mut app = App::new(None, None, None);
        let (connect_app, _) = connect::app::App::new(
            None,
            Some(crate::commands::connect::ConnectProvider::AppleCalendar),
            None,
            None,
        );
        app.overlay = Overlay::Connect(connect_app);

        let effects =
            app.handle_connect_runtime(connect::runtime::RuntimeEvent::CalendarPermissionStatus(
                connect::runtime::CalendarPermissionState::NotDetermined,
            ));

        assert!(effects.is_empty());

        match app.overlay {
            Overlay::Connect(ref connect_app) => {
                assert_eq!(
                    connect_app.calendar().auth_status(),
                    Some(connect::runtime::CalendarPermissionState::NotDetermined)
                );
            }
            Overlay::None | Overlay::Models(_) | Overlay::Meetings(_) | Overlay::Timeline(_) => {
                panic!("expected connect overlay")
            }
        }
    }

    #[test]
    fn connect_calendar_effects_are_forwarded() {
        let mut app = App::new(None, None, None);

        let effects = app.translate_connect_effects(vec![
            connect::effect::Effect::CheckCalendarPermission,
            connect::effect::Effect::LoadCalendars,
        ]);

        assert_eq!(effects.len(), 2);
        assert!(matches!(effects[0], Effect::CheckCalendarPermission));
        assert!(matches!(effects[1], Effect::LoadCalendars));
    }
}
