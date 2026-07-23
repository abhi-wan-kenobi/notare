use std::sync::Arc;

use clap::Parser;
use stt_server::{AppState, Config, build_router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();

    tracing::info!(
        host = %config.host,
        port = config.port,
        model = %config.model,
        model_dir = %config.model_dir.display(),
        require_gpu = config.require_gpu,
        // Never log the token itself — only whether the shared-secret gate
        // is on (`crate::auth`).
        token_configured = config.token.is_some(),
        "starting notare-stt-server"
    );

    if config.host == "0.0.0.0" && config.token.is_none() {
        tracing::warn!(
            "binding 0.0.0.0 with no NOTARE_STT_TOKEN configured: this server is plaintext \
             and unauthenticated on the whole LAN by design (docs/stt-server-design.md §10). \
             Do not port-forward it to the internet. Front it with Tailscale/a VPN for remote \
             access, or set NOTARE_STT_TOKEN for an extra shared-secret gate on top of LAN \
             isolation."
        );
    }

    let model_path = config.model_path();
    if !model_path.is_file() {
        tracing::warn!(
            path = %model_path.display(),
            "model file not found at startup; /health and /api/status still answer, \
             but /v1/listen will return a `model_load_failed` error until a model is installed"
        );
    }

    let host = config.host.clone();
    let port = config.port;
    let state = Arc::new(AppState::new(config));

    // Print GGML backends and verify GPU offload requirement if configured
    let backends = hypr_whisper_local::list_ggml_backends();
    if backends.is_empty() {
        tracing::info!("No GGML backends found (or running a debug build)");
    } else {
        for b in &backends {
            tracing::info!(
                kind = %b.kind,
                name = %b.name,
                description = %b.description,
                total_memory_mb = b.total_memory_mb,
                free_memory_mb = b.free_memory_mb,
                "GGML backend found"
            );
        }
    }

    let has_gpu = backends
        .iter()
        .any(|b| b.kind == "GPU" || b.kind == "ACCEL");
    if state.config.require_gpu && !has_gpu {
        tracing::error!(
            "require_gpu is set to true but no GPU or ACCEL backend is available. Exiting."
        );
        std::process::exit(1);
    }

    // Startup reconciliation (design doc §8): verify every installed
    // catalog model's on-disk reality before serving, so a half-written or
    // bit-rotted model is quarantined (`*.corrupt`) and reflected in
    // `/api/models` from the very first request instead of surfacing as a
    // mysterious `model_load_failed` later.
    state.reconcile_on_startup().await;

    // Run the startup probe if the model file is already present on disk
    let model_path = state.config.model_path();
    if model_path.is_file() {
        let state_clone = state.clone();
        tokio::spawn(async move {
            state_clone
                .run_probe_for_model(state_clone.config.model.clone())
                .await;
        });
    }

    let app = build_router(state.clone());

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!(
        addr = %listener.local_addr()?,
        "listening (GET / admin page, GET /health, GET /api/status, GET/POST /api/models*, POST+WS /v1/listen)"
    );

    // Periodic RTF health monitor (WS-G): re-measures sustained GPU throughput
    // mid-life, since the one-shot startup probe cannot see the Vulkan decay
    // that builds up under long uptime + load. Started after the server is
    // listening + the startup probe was spawned; it sleeps the interval first,
    // so the startup probe (a few seconds) finishes long before the first tick.
    // See `src/health.rs` and AGENTS.md for the bug this fixes.
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            stt_server::health::run(state_clone).await;
        });
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received, stopping");
}
