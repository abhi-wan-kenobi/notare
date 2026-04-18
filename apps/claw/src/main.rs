use anyhow::{Context, Result};
use zeroclaw_config::schema::{Config, ModelProviderConfig, TelegramConfig};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,claw=debug".into()))
        .init();

    let config = build_config()?;
    tracing::info!(
        fallback = ?config.providers.fallback,
        telegram = config.channels.telegram.is_some(),
        "claw starting"
    );

    zeroclaw_channels::orchestrator::start_channels(config).await?;
    Ok(())
}

fn build_config() -> Result<Config> {
    let mut cfg = Config::default();

    let llm_base = std::env::var("LLM_PROXY_BASE_URL")
        .context("LLM_PROXY_BASE_URL must be set (e.g. https://llm-proxy.internal/v1)")?;
    let llm_token = std::env::var("LLM_PROXY_TOKEN")
        .context("LLM_PROXY_TOKEN must be set (supabase-issued bearer)")?;

    cfg.providers.fallback = Some("openai".to_string());
    cfg.providers.models.insert(
        "openai".to_string(),
        ModelProviderConfig {
            api_key: Some(llm_token),
            base_url: Some(llm_base),
            name: Some("openai".to_string()),
            model: Some(std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into())),
            temperature: Some(0.7),
            ..Default::default()
        },
    );

    if let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") {
        cfg.channels.telegram = Some(TelegramConfig {
            enabled: true,
            bot_token,
            allowed_users: std::env::var("TELEGRAM_ALLOWED_USERS")
                .ok()
                .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                .unwrap_or_default(),
            ..Default::default()
        });
    }

    // TODO: whatsapp — requires gateway HTTPS listener for Meta webhooks.
    //   Meta → POST https://<exe.dev-url>/whatsapp (handled by zeroclaw-gateway).
    //   Populate cfg.channels.whatsapp and mount gateway routes separately.

    // TODO: supabase-auth — gate upstream in apps/api, or add axum middleware
    //   around the gateway router. No trait in zeroclaw-api to implement.

    // TODO: sandbox — exe.dev VM is already the isolation boundary, so NoopSandbox
    //   is likely right. If nested docker is supported, switch to DockerSandbox.

    Ok(cfg)
}
