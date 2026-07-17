use std::time::Instant;

use crate::config::Config;

/// Shared state for the `/api/*` admin handlers. The `/health` + `/v1/listen`
/// routes are served by `hypr_transcribe_core::TranscribeService` and do not
/// touch this state.
pub struct AppState {
    pub config: Config,
    pub start_time: Instant,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            start_time: Instant::now(),
        }
    }
}
