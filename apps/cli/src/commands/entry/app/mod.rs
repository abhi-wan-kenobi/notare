mod commands;
mod search;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hypr_cli_editor::{Editor, KeyResult};
use ratatui_image::protocol::StatefulProtocol;

use crate::commands::connect;
use crate::commands::sessions;

pub(crate) use commands::{COMMANDS, CommandEntry, SlashCommand};

use commands::{load_logo_protocol, pick_tip};
use search::command_match_score;

use super::action::Action;
use super::effect::Effect;

enum SessionsIntent {
    View,
    ChatResume,
}

pub(crate) enum Overlay {
    None,
    Connect(connect::app::App),
    Sessions(sessions::app::App),
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
    sessions_intent: SessionsIntent,
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
            sessions_intent: SessionsIntent::View,
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
            Action::SessionsLoaded(sessions) => {
                if let Overlay::Sessions(ref mut app) = self.overlay {
                    let effects = app.dispatch(sessions::action::Action::Loaded(sessions));
                    self.translate_sessions_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::SessionsLoadError(msg) => {
                if let Overlay::Sessions(ref mut app) = self.overlay {
                    let effects = app.dispatch(sessions::action::Action::LoadError(msg));
                    self.translate_sessions_effects(effects)
                } else {
                    Vec::new()
                }
            }
            Action::ConnectSaved {
                connection_type,
                provider_id,
            } => {
                match connection_type {
                    crate::cli::ConnectionType::Stt => {
                        self.stt_provider = Some(provider_id);
                    }
                    crate::cli::ConnectionType::Llm => {
                        self.llm_provider = Some(provider_id);
                    }
                }
                self.tip = pick_tip(&self.stt_provider, &self.llm_provider);
                self.status_message = Some("Provider configured".into());
                Vec::new()
            }
        }
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
                let cmd = COMMANDS.get(i)?;
                Some(CommandEntry {
                    name: cmd.name,
                    description: cmd.description,
                    group: cmd.group,
                    disabled_reason: self.disabled_reason(cmd),
                })
            })
            .collect()
    }

    fn disabled_reason(&self, cmd: &SlashCommand) -> Option<&'static str> {
        match cmd.name {
            "/listen" if self.stt_provider.is_none() => Some("no STT provider"),
            "/chat" | "/chat resume" if self.llm_provider.is_none() => Some("no LLM provider"),
            _ => None,
        }
    }

    fn is_command_disabled(&self, normalized_name: &str) -> bool {
        let name = format!("/{}", normalized_name);
        COMMANDS
            .iter()
            .find(|c| c.name == name)
            .is_some_and(|cmd| self.disabled_reason(cmd).is_some())
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
            Overlay::Sessions(ref mut app) => {
                let effects = app.dispatch(sessions::action::Action::Key(key));
                return self.translate_sessions_effects(effects);
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
            Overlay::Sessions(_) => return Vec::new(),
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

        if self.is_command_disabled(&normalized) {
            return Vec::new();
        }

        let (head, rest) = match normalized.split_once(' ') {
            Some((h, r)) => (h, r.trim()),
            None => (normalized.as_str(), ""),
        };

        match head {
            "connect" => {
                let (connect_app, initial_effects) =
                    connect::app::App::new(None, None, None, None, None, None);
                self.reset_input();
                self.overlay = Overlay::Connect(connect_app);
                self.translate_connect_effects(initial_effects)
            }
            "listen" => vec![Effect::Launch(super::EntryCommand::Listen)],
            "chat" if rest == "resume" => {
                self.reset_input();
                self.sessions_intent = SessionsIntent::ChatResume;
                self.overlay = Overlay::Sessions(sessions::app::App::new());
                vec![Effect::LoadSessions]
            }
            "chat" => vec![Effect::Launch(super::EntryCommand::Chat {
                session_id: None,
            })],
            "sessions" => {
                self.reset_input();
                self.sessions_intent = SessionsIntent::View;
                self.overlay = Overlay::Sessions(sessions::app::App::new());
                vec![Effect::LoadSessions]
            }
            "exit" | "quit" => vec![Effect::Exit],
            "auth" => {
                self.reset_input();
                vec![Effect::OpenAuth]
            }
            "bug" => {
                self.reset_input();
                vec![Effect::OpenBug]
            }
            "hello" => {
                self.reset_input();
                vec![Effect::OpenHello]
            }
            "desktop" => {
                self.reset_input();
                vec![Effect::OpenDesktop]
            }
            "model" => self.submit_model_command(rest),
            _ if head.is_empty() => Vec::new(),
            _ => {
                self.status_message = Some(format!("Unknown command: {}", command.trim()));
                Vec::new()
            }
        }
    }

    fn submit_model_command(&mut self, rest: &str) -> Vec<Effect> {
        use crate::cli::{ModelCommands, OutputFormat};

        let subcmd = rest.split_whitespace().next().unwrap_or("");
        match subcmd {
            "paths" => vec![Effect::RunModel(ModelCommands::Paths)],
            "current" => vec![Effect::RunModel(ModelCommands::Current)],
            "list" => vec![Effect::RunModel(ModelCommands::List {
                kind: None,
                supported: false,
                format: OutputFormat::Pretty,
            })],
            _ => {
                self.reset_input();
                self.status_message = Some("Usage: /model [paths | current | list]".to_string());
                Vec::new()
            }
        }
    }

    fn translate_connect_effects(&mut self, effects: Vec<connect::effect::Effect>) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                connect::effect::Effect::Save(data) => {
                    self.reset_input();
                    result.push(Effect::SaveConnect {
                        connection_type: data.connection_type,
                        provider: data.provider,
                        base_url: data.base_url,
                        api_key: data.api_key,
                    });
                }
                connect::effect::Effect::Exit => {
                    self.reset_input();
                }
            }
        }
        result
    }

    fn translate_sessions_effects(
        &mut self,
        effects: Vec<sessions::effect::Effect>,
    ) -> Vec<Effect> {
        let mut result = Vec::new();
        for effect in effects {
            match effect {
                sessions::effect::Effect::Select(id) => {
                    let cmd = match self.sessions_intent {
                        SessionsIntent::View => super::EntryCommand::View { session_id: id },
                        SessionsIntent::ChatResume => super::EntryCommand::Chat {
                            session_id: Some(id),
                        },
                    };
                    self.reset_input();
                    result.push(Effect::Launch(cmd));
                }
                sessions::effect::Effect::Exit => {
                    self.reset_input();
                }
            }
        }
        result
    }

    fn reset_input(&mut self) {
        self.overlay = Overlay::None;
        self.input = Editor::single_line();
        self.status_message = None;
        self.recompute_popup();
    }

    fn selected_command_name(&self) -> Option<&'static str> {
        let selected = *self.filtered_commands.get(self.selected_index)?;
        Some(COMMANDS.get(selected)?.name)
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
        let mut ranked = COMMANDS
            .iter()
            .enumerate()
            .filter_map(|(i, command)| {
                command_match_score(query, command.name).map(|score| (i, score))
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|(left_i, left_score), (right_i, right_score)| {
            right_score
                .cmp(left_score)
                .then_with(|| COMMANDS[*left_i].name.cmp(COMMANDS[*right_i].name))
        });

        self.filtered_commands = ranked.into_iter().map(|(i, _)| i).collect();

        use super::ui::command_popup::GROUP_ORDER;
        self.filtered_commands.sort_by_key(|&i| {
            let group = COMMANDS[i].group;
            GROUP_ORDER
                .iter()
                .position(|&g| g == group)
                .unwrap_or(usize::MAX)
        });

        if self.filtered_commands.is_empty() {
            self.filtered_commands = (0..COMMANDS.len()).collect();
        }

        self.selected_index = self
            .selected_index
            .min(self.filtered_commands.len().saturating_sub(1));
    }
}
