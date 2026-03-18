use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use tokio::sync::mpsc;

mod action;
mod app;
mod effect;
mod ui;

pub enum EntryCommand {
    Listen,
    Chat { session_id: Option<String> },
    View { session_id: String },
}

pub enum EntryAction {
    Launch(EntryCommand),
    Model(crate::cli::ModelCommands),
    Quit,
}

use self::action::Action;
use self::app::App;
use self::effect::Effect;

pub struct Args {
    pub status_message: Option<String>,
    pub initial_command: Option<String>,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
}

enum ExternalEvent {
    SessionsLoaded(Vec<hypr_db_app::SessionRow>),
    SessionsLoadError(String),
}

struct EntryScreen {
    app: App,
    external_tx: mpsc::UnboundedSender<ExternalEvent>,
}

impl EntryScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<EntryAction> {
        for effect in effects {
            match effect {
                Effect::Launch(cmd) => return ScreenControl::Exit(EntryAction::Launch(cmd)),
                Effect::LoadSessions => {
                    let tx = self.external_tx.clone();
                    let paths = crate::config::desktop::resolve_paths();
                    let db_path = paths.vault_base.join("app.db");
                    tokio::spawn(async move {
                        match crate::commands::sessions::load_sessions(db_path).await {
                            Ok(sessions) => {
                                let _ = tx.send(ExternalEvent::SessionsLoaded(sessions));
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::SessionsLoadError(e));
                            }
                        }
                    });
                }
                Effect::SaveConnect {
                    connection_type,
                    provider,
                    base_url,
                    api_key,
                } => {
                    let provider_id = provider.id().to_string();
                    let action = match crate::commands::connect::save_config(
                        crate::commands::connect::effect::SaveData {
                            connection_type,
                            provider,
                            base_url,
                            api_key,
                        },
                    ) {
                        Ok(()) => Action::ConnectSaved {
                            connection_type,
                            provider_id,
                        },
                        Err(error) => Action::StatusMessage(error.to_string()),
                    };
                    let inner = self.app.dispatch(action);
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenAuth => {
                    let message = match crate::commands::auth::run() {
                        Ok(()) => "Opened auth page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenBug => {
                    let message = match crate::commands::bug::run() {
                        Ok(()) => "Opened bug report page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenHello => {
                    let message = match crate::commands::hello::run() {
                        Ok(()) => "Opened char.com in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenDesktop => {
                    let message = match crate::commands::desktop::run() {
                        Ok(crate::commands::desktop::DesktopAction::OpenedApp) => {
                            "Opened desktop app".to_string()
                        }
                        Ok(crate::commands::desktop::DesktopAction::OpenedDownloadPage) => {
                            "Desktop app not found. Opened download page".to_string()
                        }
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::RunModel(cmd) => return ScreenControl::Exit(EntryAction::Model(cmd)),
                Effect::Exit => return ScreenControl::Exit(EntryAction::Quit),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for EntryScreen {
    type ExternalEvent = ExternalEvent;
    type Output = EntryAction;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                let effects = self.app.dispatch(Action::Paste(pasted));
                self.apply_effects(effects)
            }
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::SessionsLoaded(sessions) => Action::SessionsLoaded(sessions),
            ExternalEvent::SessionsLoadError(msg) => Action::SessionsLoadError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn on_resize(&mut self) {
        self.app.reload_logo();
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(None)
    }
}

pub async fn run(args: Args) -> EntryAction {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    let mut screen = EntryScreen {
        app: App::new(args.status_message, args.stt_provider, args.llm_provider),
        external_tx,
    };

    if let Some(command) = args.initial_command {
        let effects = screen.app.dispatch(Action::SubmitCommand(command));
        if let ScreenControl::Exit(action) = screen.apply_effects(effects) {
            return action;
        }
    }

    run_screen::<EntryScreen>(screen, Some(external_rx))
        .await
        .unwrap_or(EntryAction::Quit)
}
