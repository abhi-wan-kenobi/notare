use crate::config::paths::{self, AppPaths};
#[cfg(feature = "standalone")]
use crate::stt;

pub struct AppContext {
    analytics: hypr_analytics::AnalyticsClient,
    paths: AppPaths,
    #[cfg(feature = "standalone")]
    quiet: bool,
    trace_buffer: crate::OptTraceBuffer,
}

impl AppContext {
    pub fn new(
        base: Option<&std::path::Path>,
        quiet: bool,
        trace_buffer: crate::OptTraceBuffer,
    ) -> Self {
        #[cfg(not(feature = "standalone"))]
        let _ = quiet;

        let paths = paths::resolve_paths(base);

        Self {
            analytics: analytics_client(),
            paths,
            #[cfg(feature = "standalone")]
            quiet,
            trace_buffer,
        }
    }

    pub fn track_command(&self, subcommand: &'static str) {
        let client = self.analytics.clone();
        tokio::spawn(async move {
            let machine_id = hypr_host::fingerprint();
            let payload = hypr_analytics::AnalyticsPayload::builder("cli_command_invoked")
                .with("subcommand", subcommand)
                .with("app_identifier", "com.char.cli")
                .with(
                    "app_version",
                    option_env!("APP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
                )
                .build();
            let _ = client.event(machine_id, payload).await;
        });
    }

    #[cfg(feature = "standalone")]
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    #[cfg(feature = "standalone")]
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    #[cfg(feature = "standalone")]
    pub fn stt_overrides(
        &self,
        provider: Option<stt::SttProvider>,
        base_url: Option<String>,
        api_key: Option<String>,
        model: Option<String>,
        language: String,
    ) -> stt::SttOverrides {
        stt::SttOverrides {
            provider,
            base_url,
            api_key,
            model,
            language,
            models_base: self.paths.models_base.clone(),
        }
    }

    #[cfg(feature = "standalone")]
    pub fn trace_buffer(&self) -> crate::OptTraceBuffer {
        self.trace_buffer.clone()
    }

    #[cfg(not(feature = "standalone"))]
    pub fn trace_buffer(&self) -> crate::OptTraceBuffer {
        self.trace_buffer
    }
}

fn analytics_client() -> hypr_analytics::AnalyticsClient {
    let mut builder = hypr_analytics::AnalyticsClientBuilder::default();
    if std::env::var_os("DO_NOT_TRACK").is_none()
        && let Some(key) = option_env!("POSTHOG_API_KEY")
    {
        builder = builder.with_posthog(key);
    }
    builder.build()
}
