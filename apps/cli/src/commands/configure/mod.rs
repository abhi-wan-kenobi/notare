use clap::ValueEnum;
use hypr_cli_tui::run_screen;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConfigureTab {
    Stt,
    Llm,
    Calendar,
    Language,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Show current configuration
    Show {
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Set a configuration value
    Set {
        /// Setting key (current_stt_provider, current_llm_provider, ai_language, spoken_languages)
        key: String,
        /// Setting value
        value: String,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
}

mod action;
mod app;
mod effect;
mod runtime;
mod screen;
mod ui;

use self::app::App;
use self::runtime::Runtime;
use self::screen::ConfigureScreen;

pub async fn run(pool: &SqlitePool, cli_tab: Option<ConfigureTab>) -> CliResult<()> {
    let initial_tab = cli_tab.map(|t| match t {
        ConfigureTab::Stt => app::Tab::Stt,
        ConfigureTab::Llm => app::Tab::Llm,
        ConfigureTab::Calendar => app::Tab::Calendar,
        ConfigureTab::Language => app::Tab::Language,
    });

    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::new(pool.clone(), tx);

    let (app, initial_effects) = App::new(initial_tab);
    let mut screen = ConfigureScreen::new(app, runtime);

    screen.apply_effects(initial_effects);

    run_screen(screen, Some(rx))
        .await
        .map_err(|e| CliError::operation_failed("run configure screen", e.to_string()))
}

pub async fn run_cli(pool: &SqlitePool, command: Commands) -> CliResult<()> {
    match command {
        Commands::Show { format } => show(pool, format).await,
        Commands::Set { key, value } => {
            hypr_db_app::set_setting(pool, &key, &value)
                .await
                .map_err(|e| CliError::operation_failed("set setting", e.to_string()))?;
            eprintln!("set {key}");
            Ok(())
        }
    }
}

async fn show(pool: &SqlitePool, format: OutputFormat) -> CliResult<()> {
    let all = hypr_db_app::load_all_settings(pool)
        .await
        .map_err(|e| CliError::operation_failed("load settings", e.to_string()))?;

    let stt_providers: Vec<String> = hypr_db_app::list_connections(pool, "stt")
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.provider_id)
        .collect();
    let llm_providers: Vec<String> = hypr_db_app::list_connections(pool, "llm")
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.provider_id)
        .collect();

    let map: std::collections::HashMap<&str, &str> =
        all.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    match format {
        OutputFormat::Json => {
            let obj = serde_json::json!({
                "current_stt_provider": map.get("current_stt_provider").unwrap_or(&""),
                "current_llm_provider": map.get("current_llm_provider").unwrap_or(&""),
                "ai_language": map.get("ai_language").unwrap_or(&""),
                "spoken_languages": map.get("spoken_languages").unwrap_or(&"[]"),
                "stt_providers": stt_providers,
                "llm_providers": llm_providers,
            });
            println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        }
        OutputFormat::Pretty => {
            let current_stt = map.get("current_stt_provider").unwrap_or(&"(none)");
            let current_llm = map.get("current_llm_provider").unwrap_or(&"(none)");
            let ai_lang = map.get("ai_language").unwrap_or(&"(none)");
            let spoken = map.get("spoken_languages").unwrap_or(&"[]");

            println!("STT provider:      {current_stt}");
            if !stt_providers.is_empty() {
                println!("  available:       {}", stt_providers.join(", "));
            }
            println!("LLM provider:      {current_llm}");
            if !llm_providers.is_empty() {
                println!("  available:       {}", llm_providers.join(", "));
            }
            println!("AI language:       {ai_lang}");
            println!("Spoken languages:  {spoken}");
        }
    }
    Ok(())
}
