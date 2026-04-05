use std::time::Duration;

use clap::Parser;
use hypr_activity_capture::{CapturePolicy, PolicyMode};

const DEFAULT_POLL_INTERVAL_MS: u64 = 750;

#[derive(Debug, Clone, Copy, Parser)]
#[command(
    about = "Watch foreground activity with compact one-line output",
    long_about = None
)]
pub struct Options {
    #[arg(
        long = "poll-ms",
        default_value_t = DEFAULT_POLL_INTERVAL_MS,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub poll_ms: u64,

    #[arg(long)]
    pub metadata_only: bool,

    #[arg(long)]
    pub no_color: bool,

    #[arg(long)]
    pub once: bool,
}

impl Options {
    pub fn poll_interval(self) -> Duration {
        Duration::from_millis(self.poll_ms)
    }

    pub fn policy(self) -> CapturePolicy {
        if self.metadata_only {
            CapturePolicy::default()
        } else {
            CapturePolicy {
                mode: PolicyMode::OptOut,
                ..Default::default()
            }
        }
    }

    pub fn policy_label(self) -> &'static str {
        if self.metadata_only {
            "metadata-only"
        } else {
            "opt-out/full"
        }
    }
}
