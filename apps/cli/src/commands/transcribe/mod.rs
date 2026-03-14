pub mod core;

use std::path::PathBuf;
use std::sync::Arc;

use clap::ValueEnum;
use hypr_local_model::LocalModel;
use hypr_local_stt_server::LocalSttServer;
use owhisper_client::RealtimeSttAdapter;

use self::core::*;
use crate::commands::model::settings;
use crate::error::{CliError, CliResult, did_you_mean};

#[derive(Clone, ValueEnum)]
pub enum Provider {
    Deepgram,
    Soniox,
    Cactus,
    ProxyHyprnote,
    ProxyDeepgram,
    ProxySoniox,
}

impl Provider {
    fn is_local(&self) -> bool {
        matches!(self, Provider::Cactus)
    }
}

#[derive(clap::Args)]
pub struct TranscribeArgs {
    #[arg(long, value_enum)]
    pub provider: Provider,
    /// Model name (API model for cloud providers, model ID for local)
    #[arg(long, conflicts_with = "model_path")]
    pub model: Option<String>,
    /// Path to a local model directory on disk
    #[arg(long, conflicts_with = "model")]
    pub model_path: Option<PathBuf>,
    #[arg(long, env = "DEEPGRAM_API_KEY")]
    pub deepgram_api_key: Option<String>,
    #[arg(long, env = "SONIOX_API_KEY")]
    pub soniox_api_key: Option<String>,
    #[command(flatten)]
    pub audio: AudioArgs,
}

pub async fn run(args: TranscribeArgs) -> CliResult<()> {
    if args.model_path.is_some() && !args.provider.is_local() {
        return Err(CliError::invalid_argument_with_hint(
            "--model-path",
            args.model_path.unwrap().display().to_string(),
            "only valid with local providers (cactus)",
            "Use --provider cactus for local model files, or remove --model-path for cloud providers.",
        ));
    }

    match args.provider {
        Provider::Deepgram => {
            let api_key = require_key(args.deepgram_api_key, "DEEPGRAM_API_KEY")?;
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let mut params = default_listen_params();
            params.model = Some(model);
            run_simple_provider::<owhisper_client::DeepgramAdapter>(
                "https://api.deepgram.com/v1",
                Some(api_key),
                params,
                args.audio.audio,
            )
            .await;
        }
        Provider::Soniox => {
            let api_key = require_key(args.soniox_api_key, "SONIOX_API_KEY")?;
            let model = require_model_name(args.model.as_deref(), &args.provider)?;
            let mut params = default_listen_params();
            params.model = Some(model);
            run_simple_provider::<owhisper_client::SonioxAdapter>(
                "https://api.soniox.com",
                Some(api_key),
                params,
                args.audio.audio,
            )
            .await;
        }
        Provider::Cactus => {
            let model_path = resolve_local_model(args.model.as_deref(), args.model_path)?;
            run_cactus_from_path(model_path, args.audio.audio).await?;
        }
        Provider::ProxyHyprnote => {
            run_proxy(
                ProxyKind::Hyprnote,
                args.deepgram_api_key,
                args.soniox_api_key,
                args.audio.audio,
            )
            .await?;
        }
        Provider::ProxyDeepgram => {
            let api_key = require_key(args.deepgram_api_key, "DEEPGRAM_API_KEY")?;
            run_proxy(ProxyKind::Deepgram, Some(api_key), None, args.audio.audio).await?;
        }
        Provider::ProxySoniox => {
            let api_key = require_key(args.soniox_api_key, "SONIOX_API_KEY")?;
            run_proxy(ProxyKind::Soniox, None, Some(api_key), args.audio.audio).await?;
        }
    }
    Ok(())
}

fn require_model_name(model: Option<&str>, provider: &Provider) -> CliResult<String> {
    if let Some(m) = model {
        return Ok(m.to_string());
    }

    let hint = match provider {
        Provider::Deepgram => "Available models: nova-3, nova-2, nova, enhanced, base",
        Provider::Soniox => "Available models: stt_rt_preview",
        _ => "Pass a model name for the upstream provider.",
    };

    Err(CliError::required_argument_with_hint("--model", hint))
}

fn resolve_local_model(model_id: Option<&str>, model_path: Option<PathBuf>) -> CliResult<PathBuf> {
    if let Some(path) = model_path {
        if !path.exists() {
            return Err(CliError::not_found(
                format!("model path '{}'", path.display()),
                None,
            ));
        }
        return Ok(path);
    }

    if let Some(name) = model_id {
        return resolve_cactus_model_path(name);
    }

    Err(CliError::required_argument_with_hint(
        "--model or --model-path",
        suggest_cactus_models(),
    ))
}

fn resolve_cactus_model_path(name: &str) -> CliResult<PathBuf> {
    let paths = settings::resolve_paths();
    let models_base = &paths.models_base;

    let canonical = if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    };

    let model = LocalModel::all()
        .into_iter()
        .find(|m| {
            matches!(m, LocalModel::Cactus(_))
                && (m.cli_name() == name || m.cli_name() == canonical)
        })
        .ok_or_else(|| {
            let names: Vec<&str> = LocalModel::all()
                .iter()
                .filter(|m| matches!(m, LocalModel::Cactus(_)))
                .map(|m| m.cli_name())
                .collect();
            let mut hint = String::new();
            if let Some(suggestion) = did_you_mean(name, &names) {
                hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
            }
            hint.push_str(&suggest_cactus_models());
            CliError::not_found(format!("cactus model '{name}'"), Some(hint))
        })?;

    let path = model.install_path(models_base);
    if !path.exists() {
        return Err(CliError::not_found(
            format!("cactus model '{name}' (not downloaded)"),
            Some(format!(
                "Download it first: char model cactus download {name}"
            )),
        ));
    }

    Ok(path)
}

fn suggest_cactus_models() -> String {
    let paths = settings::resolve_paths();
    let models_base = paths.models_base;

    let mut downloaded = Vec::new();
    let mut available = Vec::new();

    for model in LocalModel::all() {
        let LocalModel::Cactus(_) = &model else {
            continue;
        };
        let name = model.cli_name();
        if model.install_path(&models_base).exists() {
            downloaded.push(name);
        } else {
            available.push(name);
        }
    }

    let mut hint = String::new();
    if !downloaded.is_empty() {
        hint.push_str("Downloaded models:\n");
        for name in &downloaded {
            hint.push_str(&format!("  {name}\n"));
        }
    }
    if !available.is_empty() {
        if !downloaded.is_empty() {
            hint.push_str("Other models (not downloaded):\n");
        } else {
            hint.push_str("No models downloaded. Available models:\n");
        }
        for name in &available {
            hint.push_str(&format!("  {name}\n"));
        }
        hint.push_str("Download with: char model cactus download <name>");
    }
    if hint.is_empty() {
        hint.push_str("No cactus models found. Run `char model cactus list` to check.");
    }
    hint
}

fn require_key(key: Option<String>, env_name: &str) -> CliResult<String> {
    key.ok_or_else(|| {
        CliError::required_argument(Box::leak(
            format!(
                "--{} (or {env_name})",
                env_name.to_lowercase().replace('_', "-")
            )
            .into_boxed_str(),
        ))
    })
}

async fn run_simple_provider<A: RealtimeSttAdapter>(
    api_base: &str,
    api_key: Option<String>,
    params: owhisper_interface::ListenParams,
    source: AudioSource,
) {
    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);

    if source.is_dual() {
        let client = build_dual_client::<A>(api_base, api_key, params).await;
        run_dual_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    } else {
        let client = build_single_client::<A>(api_base, api_key, params).await;
        run_single_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    }
}

async fn run_cactus_from_path(model_path: PathBuf, source: AudioSource) -> CliResult<()> {
    let server = LocalSttServer::start(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;
    let base_url = server.base_url().to_string();

    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);

    if source.is_dual() {
        let client = build_dual_client::<owhisper_client::CactusAdapter>(
            &base_url,
            None,
            default_listen_params(),
        )
        .await;
        run_dual_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    } else {
        let client = build_single_client::<owhisper_client::CactusAdapter>(
            &base_url,
            None,
            default_listen_params(),
        )
        .await;
        run_single_client(
            audio,
            source,
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    }

    // keep server alive until transcription ends
    drop(server);
    Ok(())
}

enum ProxyKind {
    Hyprnote,
    Deepgram,
    Soniox,
}

async fn run_proxy(
    kind: ProxyKind,
    deepgram_api_key: Option<String>,
    soniox_api_key: Option<String>,
    source: AudioSource,
) -> CliResult<()> {
    use hypr_transcribe_proxy::{HyprnoteRoutingConfig, SttProxyConfig};

    let mut env = hypr_transcribe_proxy::Env::default();
    let provider_name = match kind {
        ProxyKind::Hyprnote => {
            env.stt.deepgram_api_key = deepgram_api_key;
            env.stt.soniox_api_key = soniox_api_key;
            "hyprnote"
        }
        ProxyKind::Deepgram => {
            env.stt.deepgram_api_key = deepgram_api_key;
            "deepgram"
        }
        ProxyKind::Soniox => {
            env.stt.soniox_api_key = soniox_api_key;
            "soniox"
        }
    };

    let supabase_env = hypr_api_env::SupabaseEnv {
        supabase_url: String::new(),
        supabase_anon_key: String::new(),
        supabase_service_role_key: String::new(),
    };

    let config = SttProxyConfig::new(&env, &supabase_env)
        .with_hyprnote_routing(HyprnoteRoutingConfig::default());
    let app = hypr_transcribe_proxy::router(config);
    let server = spawn_router(app).await;

    eprintln!("proxy: {} -> {}", server.addr(), provider_name);
    eprintln!();

    let audio: Arc<dyn AudioProvider> = Arc::new(ActualAudio);
    let api_base = server.api_base("");

    match kind {
        ProxyKind::Hyprnote => {
            run_with_adapter::<owhisper_client::HyprnoteAdapter>(audio, &source, api_base).await;
        }
        ProxyKind::Deepgram => {
            run_with_adapter::<owhisper_client::DeepgramAdapter>(audio, &source, api_base).await;
        }
        ProxyKind::Soniox => {
            run_with_adapter::<owhisper_client::SonioxAdapter>(audio, &source, api_base).await;
        }
    }

    Ok(())
}

async fn run_with_adapter<A: RealtimeSttAdapter>(
    audio: Arc<dyn AudioProvider>,
    source: &AudioSource,
    api_base: String,
) {
    if source.is_dual() {
        let client = build_dual_client::<A>(api_base, None, default_listen_params()).await;
        run_dual_client(
            audio,
            source.clone(),
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    } else {
        let client = build_single_client::<A>(api_base, None, default_listen_params()).await;
        run_single_client(
            audio,
            source.clone(),
            client,
            DEFAULT_SAMPLE_RATE,
            DEFAULT_TIMEOUT_SECS,
        )
        .await;
    }
}
