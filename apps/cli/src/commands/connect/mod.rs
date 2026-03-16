mod action;
mod app;
mod effect;
mod ui;

use std::convert::Infallible;
use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use url::Url;

pub use crate::cli::{ConnectProvider, ConnectionType};
use crate::config::desktop;
use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::{App, Step};
use self::effect::Effect;

const IDLE_FRAME: Duration = Duration::from_secs(1);

const STT_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Deepgram,
    ConnectProvider::Soniox,
    ConnectProvider::Assemblyai,
    ConnectProvider::Openai,
    ConnectProvider::Gladia,
    ConnectProvider::Elevenlabs,
    ConnectProvider::Mistral,
    ConnectProvider::Fireworks,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    ConnectProvider::Cactus,
    ConnectProvider::Custom,
];

const LLM_PROVIDERS: &[ConnectProvider] = &[
    ConnectProvider::Openai,
    ConnectProvider::Anthropic,
    ConnectProvider::Openrouter,
    ConnectProvider::GoogleGenerativeAi,
    ConnectProvider::Mistral,
    ConnectProvider::AzureOpenai,
    ConnectProvider::AzureAi,
    ConnectProvider::Ollama,
    ConnectProvider::Lmstudio,
    ConnectProvider::Custom,
];

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stt => write!(f, "stt"),
            Self::Llm => write!(f, "llm"),
        }
    }
}

impl std::fmt::Display for ConnectProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl ConnectProvider {
    pub(crate) fn id(&self) -> &'static str {
        match self {
            Self::Deepgram => "deepgram",
            Self::Soniox => "soniox",
            Self::Assemblyai => "assemblyai",
            Self::Openai => "openai",
            Self::Gladia => "gladia",
            Self::Elevenlabs => "elevenlabs",
            Self::Mistral => "mistral",
            Self::Fireworks => "fireworks",
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => "cactus",
            Self::Anthropic => "anthropic",
            Self::Openrouter => "openrouter",
            Self::GoogleGenerativeAi => "google_generative_ai",
            Self::AzureOpenai => "azure_openai",
            Self::AzureAi => "azure_ai",
            Self::Ollama => "ollama",
            Self::Lmstudio => "lmstudio",
            Self::Custom => "custom",
        }
    }

    fn is_local(&self) -> bool {
        match self {
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => true,
            Self::Ollama | Self::Lmstudio => true,
            _ => false,
        }
    }

    fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Deepgram => Some("https://api.deepgram.com/v1"),
            Self::Soniox => Some("https://api.soniox.com"),
            Self::Assemblyai => Some("https://api.assemblyai.com"),
            Self::Openai => Some("https://api.openai.com/v1"),
            Self::Gladia => Some("https://api.gladia.io"),
            Self::Elevenlabs => Some("https://api.elevenlabs.io"),
            Self::Mistral => Some("https://api.mistral.ai/v1"),
            Self::Fireworks => Some("https://api.fireworks.ai"),
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Openrouter => Some("https://openrouter.ai/api/v1"),
            Self::GoogleGenerativeAi => {
                Some("https://generativelanguage.googleapis.com/v1beta")
            }
            Self::Ollama => Some("http://127.0.0.1:11434/v1"),
            Self::Lmstudio => Some("http://127.0.0.1:1234/v1"),
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            Self::Cactus => None,
            Self::AzureOpenai | Self::AzureAi | Self::Custom => None,
        }
    }

    fn valid_for(&self, ct: ConnectionType) -> bool {
        match ct {
            ConnectionType::Stt => STT_PROVIDERS.contains(self),
            ConnectionType::Llm => LLM_PROVIDERS.contains(self),
        }
    }
}

// --- Screen ---

struct ConnectScreen {
    app: App,
}

impl ConnectScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<SaveData>> {
        for effect in effects {
            match effect {
                Effect::Save {
                    connection_type,
                    provider,
                    base_url,
                    api_key,
                } => {
                    return ScreenControl::Exit(Some(SaveData {
                        connection_type,
                        provider,
                        base_url,
                        api_key,
                    }));
                }
                Effect::Exit => return ScreenControl::Exit(None),
            }
        }
        ScreenControl::Continue
    }
}

struct SaveData {
    connection_type: ConnectionType,
    provider: ConnectProvider,
    base_url: Option<String>,
    api_key: Option<String>,
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
            TuiEvent::Draw => ScreenControl::Continue,
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
        "char connect".to_string()
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

    if let (Some(ct), Some(p)) = (args.connection_type, &args.provider) {
        if !p.valid_for(ct) {
            return Err(CliError::invalid_argument(
                "--provider",
                p.id(),
                format!("not a valid {ct} provider"),
            ));
        }
    }

    if let Some(ref url) = args.base_url {
        validate_base_url(url)
            .map_err(|reason| CliError::invalid_argument("--base-url", url, reason))?;
    }

    let (app, initial_effects) = App::new(
        args.connection_type,
        args.provider,
        args.base_url,
        args.api_key,
    );

    if app.step == Step::Done {
        for effect in initial_effects {
            if let Effect::Save {
                connection_type,
                provider,
                base_url,
                api_key,
            } = effect
            {
                save_config(connection_type, provider, base_url, api_key)?;
                return Ok(true);
            }
        }
    }

    if !interactive {
        return Err(match app.step {
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
                    app.provider.map(|p| p.id()).unwrap_or("provider")
                ),
            ),
            Step::InputApiKey => CliError::required_argument_with_hint(
                "--api-key",
                "pass --api-key <key> (interactive prompts require a terminal)",
            ),
            Step::Done => unreachable!(),
        });
    }

    let screen = ConnectScreen { app };
    let result = run_screen(screen, None)
        .await
        .map_err(|e| CliError::operation_failed("connect tui", e.to_string()))?;

    match result {
        Some(data) => {
            save_config(data.connection_type, data.provider, data.base_url, data.api_key)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn validate_base_url(input: &str) -> Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    Url::parse(trimmed)
        .map(|_| ())
        .map_err(|e| format!("invalid URL: {e}"))
}

fn save_config(
    connection_type: ConnectionType,
    provider: ConnectProvider,
    base_url: Option<String>,
    api_key: Option<String>,
) -> CliResult<()> {
    let type_key = connection_type.to_string();
    let provider_id = provider.id();

    let mut provider_config = serde_json::Map::new();
    if let Some(url) = &base_url {
        provider_config.insert("base_url".into(), serde_json::Value::String(url.clone()));
    }
    if let Some(key) = &api_key {
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
