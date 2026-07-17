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
        "starting notare-stt-server"
    );

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
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening (GET /health, GET /api/status, GET /api/models, POST+WS /v1/listen)");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received, stopping");
}
