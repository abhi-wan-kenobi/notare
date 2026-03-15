mod action;
mod app;
mod effect;
mod runtime;
mod ui;

use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use hypr_openrouter::Client;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};
use crate::runtime::session_context::load_chat_system_message;

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

pub struct Args {
    pub session: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
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
                Effect::Submit { messages } => {
                    self.runtime.submit(messages);
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
            TuiEvent::Draw => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            RuntimeEvent::StreamChunk(chunk) => Action::StreamChunk(chunk),
            RuntimeEvent::StreamCompleted => Action::StreamCompleted,
            RuntimeEvent::StreamFailed(error) => Action::StreamFailed(error),
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
    let api_key = args
        .api_key
        .ok_or_else(|| CliError::required_argument_with_hint("--api-key", "set CHAR_API_KEY"))?;
    let model = args
        .model
        .unwrap_or_else(|| "anthropic/claude-sonnet-4".to_string());
    let system_message = args
        .session
        .as_deref()
        .map(load_chat_system_message)
        .transpose()?;

    let (runtime_tx, runtime_rx) = mpsc::unbounded_channel();
    let client = Client::new(api_key.clone());
    let runtime = Runtime::new(client, model.clone(), runtime_tx);
    let app = App::new(model, args.session, system_message);

    run_screen(ChatScreen::new(app, runtime), Some(runtime_rx))
        .await
        .map_err(|e| CliError::operation_failed("chat tui", e.to_string()))
}
