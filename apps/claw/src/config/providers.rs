use anyhow::{Context, Result};
use zeroclaw_config::{providers::ProvidersConfig, schema::ModelProviderConfig};

const DEV_BASE_URL: &str = "http://localhost:3001";
const PROD_BASE_URL: &str = "https://api.char.com";

pub fn configure(providers: &mut ProvidersConfig) -> Result<()> {
    let base_url =
        std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| default_base_url().to_string());
    let api_key = std::env::var("OPENAI_API_KEY")
        .context("OPENAI_API_KEY must be set (supabase-issued bearer)")?;
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());

    providers.fallback = Some("openai".to_string());
    providers.models.insert(
        "openai".to_string(),
        ModelProviderConfig {
            api_key: Some(api_key),
            base_url: Some(base_url),
            name: Some("openai".to_string()),
            model: Some(model),
            temperature: Some(0.7),
            ..Default::default()
        },
    );
    Ok(())
}

fn default_base_url() -> &'static str {
    if cfg!(debug_assertions) {
        DEV_BASE_URL
    } else {
        PROD_BASE_URL
    }
}
