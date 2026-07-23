//! Periodic RTF-based GPU health monitor (WS-G).
//!
//! The Vulkan STT backend on the RX 6600 degrades in place after long uptime +
//! sustained load: the model stays GPU-resident and "GPU offload verified", but
//! sustained transcription throughput decays from ~25x realtime to ~1.3x. A
//! plain container restart clears it. The startup GPU-offload probe runs ONCE
//! at boot, so a mid-life throughput drop is never re-detected — the service
//! silently gets slow. A boolean is-GPU-loaded check is useless here (the GPU
//! is engaged the whole time); the only thing that catches this decay is
//! periodically measuring the ACTUAL realtime factor.
//!
//! This module runs that measurement on a background tokio task: sleep the
//! interval, skip the tick if a real transcription is in flight (to avoid a
//! false positive from contention), run the same `probe::run_probe` used at
//! startup, and on sustained degradation (`fail_streak` consecutive low/failed
//! probes) log loudly, flip `GET /health` to 503, and — unless
//! `NOTARE_STT_HEALTH_AUTORESTART=false` — `exit(1)` so the container restart
//! clears the decay.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::state::AppState;

/// Pure threshold/streak hysteresis decision for one periodic probe.
///
/// Given the current consecutive-low count and a single probe result, returns
/// the new streak and whether the service is now sustained-degraded (streak
/// reached the fail threshold). Pure so it is unit-testable without a real
/// GPU/probe:
/// - `Some(rtf)` with `rtf >= min_rtf` is a healthy probe → resets streak to 0.
/// - `Some(rtf)` with `rtf < min_rtf` is a slow probe → increments the streak.
/// - `None` (the probe could not run at all) is itself unhealthy → increments.
/// - Degraded once `new_streak >= fail_streak`, so a single transient blip below
///   the default threshold of 2 does not trip it.
pub(crate) fn update_streak(
    current_streak: u32,
    probe: Option<f32>,
    min_rtf: f32,
    fail_streak: u32,
) -> (u32, bool) {
    let new_streak = match probe {
        Some(rtf) if rtf >= min_rtf => 0,
        _ => current_streak.saturating_add(1),
    };
    (new_streak, new_streak >= fail_streak)
}

/// `GET /health` — 200 "ok" while the periodic monitor considers the service
/// healthy, 503 "degraded" once it has flipped the latch on sustained RTF
/// degradation. Stays reachable with no token (registered outside the auth
/// layer, see `router::build_router`) so container HEALTHCHECKs never break.
pub async fn health_handler(state: Arc<AppState>) -> impl IntoResponse {
    if state.is_healthy() {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded")
    }
}

/// Background periodic RTF health monitor. Started from `main` after the server
/// is listening and the startup probe has been spawned. Loops forever (until
/// the process exits, either via the autorestart path or normal shutdown).
pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(state.config.health_interval_secs);
    let min_rtf = state.config.health_min_rtf;
    let fail_streak = state.config.health_fail_streak;
    let autorestart = state.config.health_autorestart;

    tracing::info!(
        interval_secs = interval.as_secs(),
        min_rtf = min_rtf,
        fail_streak = fail_streak,
        autorestart = autorestart,
        "stt_health_monitor_started"
    );

    loop {
        tokio::time::sleep(interval).await;

        // Skip under real load: a probe contending with an active job would
        // measure contention, not the GPU's sustained throughput, and trip a
        // false positive. The process-global activity registry's `current`
        // session is set by both the batch and streaming `/v1/listen` paths
        // (via `activity::begin_guarded`) and is what `/dashboard` already uses
        // to tell "No active transcription" from a live one, so reuse it rather
        // than adding a parallel job counter.
        if hypr_transcribe_core::activity::registry()
            .snapshot()
            .current
            .is_some()
        {
            tracing::debug!("stt_health_skip: a transcription is in flight");
            continue;
        }

        let probe = crate::probe::run_probe(state.config.port, state.config.token.as_deref()).await;

        // Record the latest periodic RTF for `/api/status` (null if the probe
        // could not run).
        *state.periodic_probe_result.write().await = probe;

        let prev_streak = state.low_streak();
        let (new_streak, degraded) = update_streak(prev_streak, probe, min_rtf, fail_streak);
        state.set_low_streak(new_streak);

        if !degraded {
            tracing::debug!(
                periodic_realtime_factor = ?probe,
                streak = new_streak,
                "stt_health_ok"
            );
            continue;
        }

        // Only act on the transition into degradation. The `healthy` flag
        // doubles as the "already tripped" latch: with autorestart on we exit on
        // the first trip, and with it off we leave `/health` at 503 for an
        // operator to decide instead of re-logging/re-exiting every tick.
        if !state.is_healthy() {
            continue;
        }
        state.set_unhealthy();
        tracing::error!(
            periodic_realtime_factor = ?probe,
            streak = new_streak,
            min_rtf = min_rtf,
            fail_streak = fail_streak,
            "stt_health_degraded"
        );
        if autorestart {
            tracing::error!(
                "stt_health_autorestart: exiting so the container restart clears the Vulkan throughput decay"
            );
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spec's worked example with defaults (min_rtf=5.0, fail_streak=2):
    /// a healthy probe resets, then two consecutive low probes trip degraded
    /// on the second one.
    #[test]
    fn streak_trips_after_two_consecutive_lows() {
        let (s0, d0) = update_streak(0, Some(20.0), 5.0, 2);
        assert_eq!((s0, d0), (0, false));

        let (s1, d1) = update_streak(s0, Some(2.0), 5.0, 2);
        assert_eq!((s1, d1), (1, false));

        let (s2, d2) = update_streak(s1, Some(2.0), 5.0, 2);
        assert_eq!((s2, d2), (2, true));
    }

    /// A single transient low blip must NOT trip degradation at the default
    /// streak of 2 — it increments once and the next healthy probe resets it.
    #[test]
    fn single_low_blip_does_not_trip() {
        let (s, d) = update_streak(0, Some(1.3), 5.0, 2);
        assert_eq!((s, d), (1, false));
        let (s, d) = update_streak(s, Some(25.0), 5.0, 2);
        assert_eq!((s, d), (0, false));
    }

    /// A probe that fails to run (`None`) is itself unhealthy and increments
    /// the streak — a probe that can't run is a degraded signal, not neutral.
    #[test]
    fn none_probe_increments_streak() {
        let (s, d) = update_streak(0, None, 5.0, 2);
        assert_eq!((s, d), (1, false));
        let (s, d) = update_streak(s, None, 5.0, 2);
        assert_eq!((s, d), (2, true));
    }

    /// A healthy probe at exactly the threshold counts as healthy (>=).
    #[test]
    fn boundary_rtf_is_healthy() {
        let (s, d) = update_streak(1, Some(5.0), 5.0, 2);
        assert_eq!((s, d), (0, false));
    }

    /// A low probe followed by a healthy probe followed by two lows trips
    /// only on the final second consecutive low — hysteresis resets cleanly.
    #[test]
    fn healthy_probe_resets_streak_mid_sequence() {
        let (s, _) = update_streak(0, Some(2.0), 5.0, 2); // streak 1
        let (s, _) = update_streak(s, Some(2.0), 5.0, 2); // streak 2, degraded
        let (s, d) = update_streak(s, Some(30.0), 5.0, 2); // reset to 0
        assert_eq!((s, d), (0, false));
        let (s, d) = update_streak(s, Some(2.0), 5.0, 2); // streak 1
        assert_eq!((s, d), (1, false));
        let (s, d) = update_streak(s, Some(2.0), 5.0, 2); // streak 2
        assert_eq!((s, d), (2, true));
    }

    /// A higher fail_streak needs that many consecutive lows to trip.
    #[test]
    fn higher_fail_streak_needs_more_lows() {
        let mut s = 0;
        let mut degraded = false;
        for _ in 0..4 {
            (s, degraded) = update_streak(s, Some(2.0), 5.0, 5);
            assert!(!degraded, "should not trip before 5 consecutive lows");
        }
        (s, degraded) = update_streak(s, Some(2.0), 5.0, 5);
        assert!(degraded);
        assert_eq!(s, 5);
    }

    /// Streak saturates instead of overflowing on a long run of lows.
    #[test]
    fn streak_saturates() {
        let mut s = u32::MAX - 1;
        let (_, degraded) = update_streak(s, None, 5.0, 2);
        let _ = degraded;
        let (next, _) = update_streak(s, None, 5.0, 2);
        assert_eq!(next, u32::MAX);
    }
}
