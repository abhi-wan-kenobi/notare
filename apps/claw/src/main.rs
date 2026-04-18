mod config;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let cfg = config::build()?;
    tracing::info!(
        fallback = ?cfg.providers.fallback,
        telegram = cfg.channels.telegram.is_some(),
        "claw starting"
    );

    zeroclaw_channels::orchestrator::start_channels(cfg).await?;
    Ok(())
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,claw=debug".into()))
        .init();
}
