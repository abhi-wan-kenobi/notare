use zeroclaw_config::schema::{ChannelsConfig, TelegramConfig};

pub fn configure(channels: &mut ChannelsConfig) {
    channels.telegram = telegram();

    // TODO: whatsapp — requires gateway HTTPS listener for Meta webhooks.
    //   Meta → POST https://<exe.dev-url>/whatsapp (handled by zeroclaw-gateway).
    //   Populate cfg.channels.whatsapp and mount gateway routes separately.

    // TODO: supabase-auth — gate upstream in apps/api, or add axum middleware
    //   around the gateway router. No trait in zeroclaw-api to implement.

    // TODO: sandbox — exe.dev VM is already the isolation boundary, so NoopSandbox
    //   is likely right. If nested docker is supported, switch to DockerSandbox.
}

fn telegram() -> Option<TelegramConfig> {
    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok()?;
    Some(TelegramConfig {
        enabled: true,
        bot_token,
        allowed_users: std::env::var("TELEGRAM_ALLOWED_USERS")
            .ok()
            .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
            .unwrap_or_default(),
        ..Default::default()
    })
}
