use anyhow::Result;
use zeroclaw_config::schema::Config;

mod channels;
mod providers;

pub fn build() -> Result<Config> {
    let mut cfg = Config::default();
    providers::configure(&mut cfg.providers)?;
    channels::configure(&mut cfg.channels);
    Ok(cfg)
}
