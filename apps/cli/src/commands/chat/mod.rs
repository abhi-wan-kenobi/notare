mod action;
mod app;
mod effect;
mod runtime;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use tokio::sync::mpsc;

use crate::config::session_context::load_chat_system_message;
use crate::error::{CliError, CliResult};
use crate::llm::{LlmProvider, resolve_config};

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

pub struct Args {
    pub session: Option<String>,
    pub prompt: Option<String>,
    pub provider: Option<LlmProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub db_path: PathBuf,
    pub resume_session_id: Option<String>,
}

struct ChatScreen {
    app: App,
    runtime: Runtime,
}

impl ChatScreen {
    fn new(app: App, runtime: Runtime) -> Self {
        Self { app, runtime }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Submit { prompt, history } => {
                    self.runtime.submit(prompt, history);
                }
                Effect::GenerateTitle { prompt, response } => {
                    self.runtime.generate_title(prompt, response);
                }
                Effect::Persist {
                    db_path,
                    session_id,
                    message_id,
                    role,
                    content,
                } => {
                    self.runtime
                        .persist_message(db_path, session_id, message_id, role, content);
                }
                Effect::UpdateTitle {
                    db_path,
                    session_id,
                    title,
                } => {
                    self.runtime.update_title(db_path, session_id, title);
                }
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for ChatScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = ();

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
            RuntimeEvent::Chunk(chunk) => Action::StreamChunk(chunk),
            RuntimeEvent::Completed(final_text) => Action::StreamCompleted(final_text),
            RuntimeEvent::Failed(error) => Action::StreamFailed(error),
            RuntimeEvent::TitleGenerated(title) => Action::TitleGenerated(title),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let system_message = args
        .session
        .as_deref()
        .map(load_chat_system_message)
        .transpose()?;
    let config = resolve_config(args.provider, args.base_url, args.api_key, args.model)?;

    if let Some(prompt) = args.prompt {
        return crate::agent::run_prompt(config, system_message, &prompt).await;
    }

    let db_path = args.db_path;
    let session_id = args
        .resume_session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let (runtime_tx, runtime_rx) = mpsc::unbounded_channel();
    let runtime = Runtime::new(config.clone(), system_message, runtime_tx)?;

    let mut app = App::new(
        config.model,
        args.session,
        db_path.clone(),
        session_id.clone(),
    );

    // Load existing messages if resuming, otherwise create new session
    let history = load_or_create_session(&db_path, &session_id).await;
    if let Some(messages) = history {
        app.load_history(messages);
    } else {
        runtime.create_session(db_path, session_id);
    }

    run_screen(ChatScreen::new(app, runtime), Some(runtime_rx))
        .await
        .map_err(|e| CliError::operation_failed("chat tui", e.to_string()))
}

async fn load_or_create_session(
    db_path: &std::path::Path,
    session_id: &str,
) -> Option<Vec<hypr_db_app::ChatMessageRow>> {
    let db = hypr_db_core2::Db3::connect_local_plain(db_path)
        .await
        .ok()?;
    hypr_db_app::migrate(db.pool()).await.ok()?;
    let session = hypr_db_app::get_session(db.pool(), session_id).await.ok()?;
    if session.is_some() {
        Some(
            hypr_db_app::load_chat_messages(db.pool(), session_id)
                .await
                .unwrap_or_default(),
        )
    } else {
        None
    }
}
