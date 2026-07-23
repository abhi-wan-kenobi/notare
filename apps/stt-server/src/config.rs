use std::path::PathBuf;

use clap::Parser;
use hypr_local_model::{LocalModel, WhisperModel};

/// Default port for the companion server (adopted decision, see
/// `docs/stt-server-design.md` §12 open question 1).
pub const DEFAULT_PORT: u16 = 8383;

/// Default default-model, matching the desktop's smallest general-purpose
/// multilingual model.
const DEFAULT_MODEL: WhisperModel = WhisperModel::QuantizedSmall;

/// Notare STT companion server configuration.
///
/// Every field is settable via a CLI flag or the matching env var (env wins
/// only when the flag is absent, standard clap precedence: CLI > env >
/// default).
#[derive(Debug, Clone, Parser)]
#[command(
    name = "stt-server",
    version,
    about = "Notare STT companion server (LAN whisper.cpp transcription, Deepgram-compatible /v1/listen)"
)]
pub struct Config {
    /// Address to bind to. `0.0.0.0` serves the whole LAN; use `127.0.0.1`
    /// to restrict to loopback.
    #[arg(long, env = "NOTARE_STT_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// Port to bind to.
    #[arg(long, env = "NOTARE_STT_PORT", default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Base directory for the model catalog layout. A model installs at
    /// `<model_dir>/stt/<file name>` (mirrors the desktop's `models_base`,
    /// see `docs/stt-server-design.md` §8).
    #[arg(long, env = "NOTARE_STT_MODEL_DIR", default_value = "./data/models")]
    pub model_dir: PathBuf,

    /// Which whisper.cpp model to serve on `/v1/listen`.
    #[arg(long, env = "NOTARE_STT_MODEL", default_value = "QuantizedSmall")]
    pub model: WhisperModel,

    /// Refuse to start unless a GPU backend is verified. No-op in Phase 1
    /// (this image only ever serves on CPU); wired now so the config/env
    /// surface does not change shape when Phase 4 adds GPU images and the
    /// offload-verification probe.
    #[arg(long, env = "NOTARE_STT_REQUIRE_GPU", default_value_t = false)]
    pub require_gpu: bool,

    /// Optional shared-secret gate (`src/auth.rs`; SEC hardening, see
    /// `docs/stt-server-design.md` §10 / `SECURITY-REVIEW.md`) enforced on
    /// `/v1/listen` and the `/api/models/*` mutation routes. Unset (`None`)
    /// by default — this server is LAN-only and unauthenticated **by
    /// design** (see the README's security warning); set this only if you
    /// want an extra shared-secret gate on top of that (e.g. a shared
    /// flat/office LAN). No desktop-side code change is needed to use it:
    /// the existing "Custom" STT provider's optional `api_key` field
    /// already sends `Authorization: Bearer <api_key>` on every request
    /// (batch, live, and the Test-connection probe) — set the same value in
    /// both places.
    #[arg(long, env = "NOTARE_STT_TOKEN")]
    pub token: Option<String>,

    /// Periodic RTF health monitor interval in seconds (WS-G). The monitor
    /// sleeps this long, then — only when no real transcription is in flight
    /// — runs the same `probe::run_probe` used at startup to re-measure
    /// sustained GPU throughput, since the one-shot startup probe cannot see
    /// the Vulkan throughput decay that builds up under long uptime + load
    /// (see `src/health.rs`).
    #[arg(long, env = "NOTARE_STT_HEALTH_INTERVAL_SECS", default_value_t = 300)]
    pub health_interval_secs: u64,

    /// Minimum realtime factor a periodic probe must sustain to count as
    /// healthy. Below this the consecutive-low streak increments; at or above
    /// it the streak resets to 0.
    #[arg(long, env = "NOTARE_STT_HEALTH_MIN_RTF", default_value_t = 5.0)]
    pub health_min_rtf: f32,

    /// Consecutive low/failed periodic probes required to declare sustained
    /// degradation (flip `/health` to 503 +, unless autorestart is off, exit).
    /// 2 = one transient blip does not trip it.
    #[arg(long, env = "NOTARE_STT_HEALTH_FAIL_STREAK", default_value_t = 2)]
    pub health_fail_streak: u32,

    /// On sustained degradation, exit(1) so `restart: unless-stopped` clears
    /// the Vulkan decay. Default true; set `NOTARE_STT_HEALTH_AUTORESTART=false`
    /// to leave `/health` at 503 for an operator to decide instead.
    #[arg(long, env = "NOTARE_STT_HEALTH_AUTORESTART", default_value_t = true)]
    pub health_autorestart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: DEFAULT_PORT,
            model_dir: PathBuf::from("./data/models"),
            model: DEFAULT_MODEL,
            require_gpu: false,
            token: None,
            health_interval_secs: 300,
            health_min_rtf: 5.0,
            health_fail_streak: 2,
            health_autorestart: true,
        }
    }
}

impl Config {
    /// Resolve the on-disk path of the configured model, using the same
    /// `<models_base>/stt/<file name>` layout the desktop and downloader
    /// crates already agree on (`LocalModel::install_path`).
    pub fn model_path(&self) -> PathBuf {
        LocalModel::Whisper(self.model.clone()).install_path(&self.model_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn defaults_match_adopted_decisions() {
        let config = Config::parse_from(["stt-server"]);
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.port, 8383);
        assert_eq!(config.model, WhisperModel::QuantizedSmall);
        assert!(!config.require_gpu);
        assert_eq!(config.model_dir, PathBuf::from("./data/models"));
        // Off by default: this server is LAN-only/unauthenticated by
        // design, not "auth required until configured".
        assert!(config.token.is_none());
    }

    #[test]
    fn token_flag_and_env_both_set_it() {
        let config = Config::parse_from(["stt-server", "--token", "s3cr3t"]);
        assert_eq!(config.token.as_deref(), Some("s3cr3t"));

        let config = Config::try_parse_from(["stt-server"]).unwrap();
        assert!(config.token.is_none());
    }

    #[test]
    fn cli_flags_override_defaults() {
        let config = Config::parse_from([
            "stt-server",
            "--host",
            "127.0.0.1",
            "--port",
            "9000",
            "--model-dir",
            "/data/models",
            "--model",
            "QuantizedLargeTurbo",
            "--require-gpu",
        ]);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.model_dir, PathBuf::from("/data/models"));
        assert_eq!(config.model, WhisperModel::QuantizedLargeTurbo);
        assert!(config.require_gpu);
    }

    #[test]
    fn model_path_follows_the_shared_stt_catalog_layout() {
        let config = Config::parse_from([
            "stt-server",
            "--model-dir",
            "/data/models",
            "--model",
            "QuantizedSmall",
        ]);
        assert_eq!(
            config.model_path(),
            PathBuf::from("/data/models/stt/ggml-small-q8_0.bin")
        );
    }

    #[test]
    fn rejects_unknown_model_ids() {
        let result = Config::try_parse_from(["stt-server", "--model", "not-a-real-model"]);
        assert!(result.is_err());
    }

    /// Every field must be reachable by its documented env var, so operators
    /// (and the Dockerfile) can configure the server without CLI flags.
    #[test]
    fn every_field_has_the_documented_env_var() {
        let command = Config::command();
        let expected: &[(&str, &str)] = &[
            ("host", "NOTARE_STT_HOST"),
            ("port", "NOTARE_STT_PORT"),
            ("model_dir", "NOTARE_STT_MODEL_DIR"),
            ("model", "NOTARE_STT_MODEL"),
            ("require_gpu", "NOTARE_STT_REQUIRE_GPU"),
            ("token", "NOTARE_STT_TOKEN"),
        ];

        for (id, env_var) in expected {
            let argument = command
                .get_arguments()
                .find(|argument| argument.get_id() == id)
                .unwrap_or_else(|| panic!("missing arg `{id}`"));
            assert_eq!(
                argument.get_env().and_then(|value| value.to_str()),
                Some(*env_var),
                "arg `{id}` is not wired to `{env_var}`"
            );
        }
    }
}
