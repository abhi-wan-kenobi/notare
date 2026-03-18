pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
mod providers;
pub(crate) mod ui;

use std::convert::Infallible;
use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

pub use crate::cli::{ConnectProvider, ConnectionType};
use crate::config::desktop;
use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::{App, Step};
use self::effect::{Effect, SaveData};

const IDLE_FRAME: Duration = Duration::from_secs(1);

// --- Screen ---

struct ConnectScreen {
    app: App,
}

impl ConnectScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<SaveData>> {
        for effect in effects {
            match effect {
                Effect::Save(data) => return ScreenControl::Exit(Some(data)),
                Effect::Exit => return ScreenControl::Exit(None),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ConnectScreen {
    type ExternalEvent = Infallible;
    type Output = Option<SaveData>;

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
            TuiEvent::Paste(text) => {
                let effects = self.app.dispatch(Action::Paste(text));
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
        match event {}
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("connect"))
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}

// --- Public API ---

pub struct Args {
    pub connection_type: Option<ConnectionType>,
    pub provider: Option<ConnectProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub async fn run(args: Args) -> CliResult<bool> {
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());

    if let (Some(ct), Some(p)) = (args.connection_type, &args.provider)
        && !p.valid_for(ct)
    {
        return Err(CliError::invalid_argument(
            "--provider",
            p.id(),
            format!("not a valid {ct} provider"),
        ));
    }

    if let Some(ref url) = args.base_url {
        app::validate_base_url(url)
            .map_err(|reason| CliError::invalid_argument("--base-url", url, reason))?;
    }

    let paths = desktop::resolve_paths();
    let settings = desktop::load_settings(&paths.settings_path);

    let (app, initial_effects) = App::new(
        args.connection_type,
        args.provider,
        args.base_url,
        args.api_key,
        settings
            .as_ref()
            .and_then(|s| s.current_llm_provider.clone()),
        settings
            .as_ref()
            .and_then(|s| s.current_stt_provider.clone()),
    );

    let save_data = if app.step() == Step::Done {
        initial_effects.into_iter().find_map(|e| match e {
            Effect::Save(data) => Some(data),
            _ => None,
        })
    } else if !interactive {
        return Err(match app.step() {
            Step::SelectType => CliError::required_argument_with_hint(
                "--type",
                "pass --type stt or --type llm (interactive prompts require a terminal)",
            ),
            Step::SelectProvider => CliError::required_argument_with_hint(
                "--provider",
                "pass --provider <name> (interactive prompts require a terminal)",
            ),
            Step::InputBaseUrl => CliError::required_argument_with_hint(
                "--base-url",
                format!(
                    "{} requires a base URL",
                    app.provider().map(|p| p.id()).unwrap_or("provider")
                ),
            ),
            Step::InputApiKey => CliError::required_argument_with_hint(
                "--api-key",
                "pass --api-key <key> (interactive prompts require a terminal)",
            ),
            Step::Done => unreachable!(),
        });
    } else {
        let screen = ConnectScreen { app };
        run_screen(screen, None)
            .await
            .map_err(|e| CliError::operation_failed("connect tui", e.to_string()))?
    };

    match save_data {
        Some(data) => {
            save_config(data)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub(crate) fn save_config(data: SaveData) -> CliResult<()> {
    let type_key = data.connection_type.to_string();
    let provider_id = data.provider.id();

    let mut provider_config = serde_json::Map::new();
    if let Some(url) = &data.base_url {
        provider_config.insert("base_url".into(), serde_json::Value::String(url.clone()));
    }
    if let Some(key) = &data.api_key {
        provider_config.insert("api_key".into(), serde_json::Value::String(key.clone()));
    }

    let patch = serde_json::json!({
        "ai": {
            format!("current_{type_key}_provider"): provider_id,
            &type_key: {
                provider_id: provider_config,
            }
        }
    });

    let paths = desktop::resolve_paths();
    desktop::save_settings(&paths.settings_path, patch)
        .map_err(|e| CliError::operation_failed("save settings", e.to_string()))?;

    eprintln!(
        "Saved {type_key} provider: {provider_id} -> {}",
        paths.settings_path.display()
    );
    Ok(())
}
