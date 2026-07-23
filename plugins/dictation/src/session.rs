//! The dictation session: default microphone -> local STT -> final transcript
//! segments injected into the focused app.
//!
//! Deliberately lighter than the meeting capture pipeline
//! (`crates/listener-core`): mic-only, nothing written to disk, no note or
//! transcript rows created. Two serving paths, matching how the meeting
//! listener dispatches (`crates/listener-core/src/actors/listener/adapters.rs`):
//!
//! - Whisper/Parakeet-ONNX/Voxtral (Windows/Linux/macOS) and Am (macOS,
//!   `char-sidecar-stt`) models are served over the internal whisper-server
//!   websocket (`/v1/listen`); the renderer passes that server's base URL and
//!   model, resolved via the local-stt plugin, and we speak to it with the
//!   generic `owhisper_client::ListenClient`.
//! - Soniqo models (macOS-only, e.g. `soniqo-parakeet-streaming`) have no WS
//!   server behind `base_url` - the local-stt plugin reports the sentinel
//!   `hypr_transcribe_soniqo::LOCAL_BASE_URL` ("soniqo://local") because
//!   transcription runs in-process through a Swift bridge. Those go through
//!   `owhisper_client::LocalSoniqoLiveClient` instead - the same client the
//!   meeting listener uses for live Soniqo transcription.

use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::{HyprnoteAdapter, ListenClient, hypr_ws_client};
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};
use tauri::Manager;
use tauri_specta::Event;

use crate::error::Error;
use crate::events::{
    DictationAmplitudeEvent, DictationFinishedEvent, DictationOutputMode, DictationPhase,
    DictationStateEvent, DictationTranscriptEvent,
};
use crate::orb;

/// Matches the whisper server's expected input (`TARGET_SAMPLE_RATE`,
/// `crates/transcribe-core/src/audio.rs`).
const SAMPLE_RATE: u32 = 16_000;
/// Throttle for the 10 Hz orb state broadcast (same cadence as the meeting
/// pipeline's `AmplitudeEmitter`). Drives `DictationStateEvent`, which carries
/// lifecycle/phase and mode - a slow re-render channel.
const AMPLITUDE_INTERVAL: Duration = Duration::from_millis(100);
/// Throttle for the high-frequency (~30 Hz) `DictationAmplitudeEvent` channel.
/// 33 ms ≈ 30.3 Hz. Separate from `AMPLITUDE_INTERVAL` so the orb ring can be
/// driven smoothly without pushing phase/state (and its re-renders) at 30 Hz.
const AMPLITUDE_FAST_INTERVAL: Duration = Duration::from_millis(33);
/// How long to wait for the server to flush segments after a Finalize.
// Must cover the server-side flush: the STT server's WS send timeout is 20s
// (transcribe-core WS_SEND_TIMEOUT) and it packs VAD chunks up to ~25s before
// the final transcript arrives. The old 5s here fired mid-flush and truncated
// the last segments (leaving the session Idle with a partial transcript).
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ActiveSession {
    stop_tx: tokio::sync::oneshot::Sender<()>,
}

#[derive(Default)]
pub struct SessionState(Mutex<Option<ActiveSession>>);

impl SessionState {
    fn take(&self) -> Option<ActiveSession> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

pub fn is_running(app: &tauri::AppHandle<tauri::Wry>) -> bool {
    let state = app.state::<SessionState>();
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    guard.is_some()
}

pub async fn start(
    base_url: String,
    model: String,
    mode: DictationOutputMode,
) -> Result<(), Error> {
    let app = orb::app_handle()?.clone();

    {
        let state = app.state::<SessionState>();
        let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            // Toggle races (hotkey + click) can double-start; make it a no-op.
            tracing::debug!("dictation session already running; ignoring start");
            return Ok(());
        }
    }

    let audio = app
        .state::<std::sync::Arc<dyn hypr_audio::AudioProvider>>()
        .inner()
        .clone();
    let chunk_size = hypr_audio_utils::chunk_size_for_stt(SAMPLE_RATE);
    let mic = audio
        .open_mic_capture(None, SAMPLE_RATE, chunk_size)
        .map_err(|e| Error::Audio(e.to_string()))?;

    let (audio_tx, audio_rx) =
        tokio::sync::mpsc::channel::<MixedMessage<bytes::Bytes, ControlMessage>>(32);
    let outbound = tokio_stream::wrappers::ReceiverStream::new(audio_rx);

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

    match resolve_soniqo_model(&base_url, &model)? {
        Some(soniqo_model) => {
            // In-process Swift bridge - no WS handshake, so no connect_policy
            // to configure (see the module doc comment).
            let client = owhisper_client::LocalSoniqoLiveClient::new(soniqo_model);
            let (listen_stream, ws_handle) = client
                .from_realtime_audio_single(
                    outbound,
                    hypr_transcribe_soniqo::TranscriptSource::Microphone,
                )
                .await
                .map_err(|e| Error::Session(format!("soniqo_live_start_failed: {e}")))?;

            commit_and_run(
                app,
                mic,
                audio_tx,
                listen_stream,
                ws_handle,
                stop_tx,
                stop_rx,
                mode,
            );
        }
        None => {
            let params = ListenParams {
                model: Some(model),
                sample_rate: SAMPLE_RATE,
                custom_query: Some(std::collections::HashMap::from([(
                    "redemption_time_ms".to_string(),
                    "400".to_string(),
                )])),
                ..Default::default()
            };

            let client = ListenClient::builder()
                .adapter::<HyprnoteAdapter>()
                .api_base(base_url)
                .api_key(String::new())
                .params(params)
                .connect_policy(hypr_ws_client::client::WebSocketConnectPolicy {
                    connect_timeout: Duration::from_secs(4),
                    max_attempts: 2,
                    retry_delay: Duration::from_secs(1),
                })
                .build_single()
                .await;

            let (listen_stream, ws_handle) = client
                .from_realtime_audio(outbound)
                .await
                .map_err(|e| Error::Session(format!("listen_ws_connect_failed: {e:?}")))?;

            commit_and_run(
                app,
                mic,
                audio_tx,
                listen_stream,
                ws_handle,
                stop_tx,
                stop_rx,
                mode,
            );
        }
    }

    Ok(())
}

/// Shared tail of `start()` once a listen stream + finalize handle have been
/// obtained (from either serving path): register the session so `stop()`/
/// `is_running()` see it, flip the orb to listening and hand the stream off
/// to `run_session`.
#[allow(clippy::too_many_arguments)]
fn commit_and_run<E: std::fmt::Debug + Send + 'static>(
    app: tauri::AppHandle<tauri::Wry>,
    mic: hypr_audio::CaptureStream,
    audio_tx: tokio::sync::mpsc::Sender<MixedMessage<bytes::Bytes, ControlMessage>>,
    listen_stream: impl futures_util::Stream<Item = Result<StreamResponse, E>> + Send + 'static,
    ws_handle: impl owhisper_client::FinalizeHandle + 'static,
    stop_tx: tokio::sync::oneshot::Sender<()>,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
    mode: DictationOutputMode,
) {
    {
        let state = app.state::<SessionState>();
        let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(ActiveSession { stop_tx });
    }

    emit_state(&app, DictationPhase::Listening, 0.0, mode);

    tauri::async_runtime::spawn(run_session(
        app,
        mic,
        audio_tx,
        listen_stream,
        ws_handle,
        stop_rx,
        mode,
    ));
}

/// Whether `(base_url, model)` names a local Soniqo model (macOS-only,
/// in-process Swift bridge - no WS server exists at `base_url`). Mirrors
/// `soniqo_model_for_args` in the meeting listener
/// (`crates/listener-core/src/actors/listener/adapters.rs`): the normal case
/// is `hypr_transcribe_soniqo::local_model_from_request` matching directly;
/// the fallback catches a request that is unambiguously local (the
/// `soniqo://local` sentinel base URL) but whose model id failed to parse,
/// so it surfaces as a clear error instead of silently falling through to a
/// WS connect attempt against a non-URL that would just hang/fail opaquely.
fn resolve_soniqo_model(
    base_url: &str,
    model: &str,
) -> Result<Option<hypr_transcribe_soniqo::SoniqoModel>, Error> {
    if let Some(model) = hypr_transcribe_soniqo::local_model_from_request(base_url, model) {
        return Ok(Some(model));
    }

    if hypr_transcribe_soniqo::is_local_base_url(base_url) {
        return model
            .parse::<hypr_transcribe_soniqo::SoniqoModel>()
            .map(Some)
            .map_err(|e| Error::Session(format!("soniqo_model_invalid: {e}")));
    }

    Ok(None)
}

pub fn stop(app: &tauri::AppHandle<tauri::Wry>) {
    let state = app.state::<SessionState>();
    if let Some(session) = state.take() {
        // The session task observes the sender drop even if send fails.
        let _ = session.stop_tx.send(());
    }
}

async fn run_session<E: std::fmt::Debug>(
    app: tauri::AppHandle<tauri::Wry>,
    mut mic: hypr_audio::CaptureStream,
    audio_tx: tokio::sync::mpsc::Sender<MixedMessage<bytes::Bytes, ControlMessage>>,
    listen_stream: impl futures_util::Stream<Item = Result<StreamResponse, E>>,
    ws_handle: impl owhisper_client::FinalizeHandle,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    mode: DictationOutputMode,
) {
    futures_util::pin_mut!(listen_stream);

    let mut finalizing = false;
    let mut failed = false;
    let mut typed_any = false;
    // The raw transcript of the whole session. In batch mode this is the
    // text delivered at the end; in type mode it is what the history entry
    // is built from (segments were already typed live).
    let mut transcript = String::new();
    let mut last_amplitude_at = std::time::Instant::now() - AMPLITUDE_INTERVAL;
    let mut last_amplitude_fast_at = std::time::Instant::now() - AMPLITUDE_FAST_INTERVAL;

    let finalize_deadline = tokio::time::sleep(Duration::from_secs(86_400));
    tokio::pin!(finalize_deadline);

    loop {
        tokio::select! {
            _ = &mut stop_rx, if !finalizing => {
                finalizing = true;
                emit_state(&app, DictationPhase::Processing, 0.0, mode);
                ws_handle.finalize().await;
                finalize_deadline
                    .as_mut()
                    .reset(tokio::time::Instant::now() + FINALIZE_TIMEOUT);
            }
            frame = mic.next(), if !finalizing => {
                match frame {
                    Some(Ok(frame)) => {
                        let samples = frame.preferred_mic();

                        // Two independent amplitude channels share the same RMS
                        // value: the dense ~30 Hz `DictationAmplitudeEvent` (orb
                        // ring) and the 10 Hz `DictationStateEvent` (phase/state
                        // + re-renders). Compute the level once when either is
                        // due; skip the work entirely on frames where neither is.
                        let now = std::time::Instant::now();
                        let fast_due = should_emit(now, last_amplitude_fast_at, AMPLITUDE_FAST_INTERVAL);
                        let state_due = should_emit(now, last_amplitude_at, AMPLITUDE_INTERVAL);
                        if fast_due || state_due {
                            let amplitude = normalized_amplitude(&samples);
                            if fast_due {
                                last_amplitude_fast_at = now;
                                emit_amplitude(&app, amplitude);
                            }
                            if state_due {
                                last_amplitude_at = now;
                                emit_state(&app, DictationPhase::Listening, amplitude, mode);
                            }
                        }

                        let bytes = hypr_audio_utils::f32_to_i16_bytes(samples.iter().copied());
                        if audio_tx.send(MixedMessage::Audio(bytes)).await.is_err() {
                            // The audio forward channel to the STT socket closed
                            // (observed on Windows when the WS write task drops
                            // mid-session). Do NOT treat this as a hard failure:
                            // that path set failed=true and broke BEFORE any
                            // finalize(), which turned the orb red, degraded
                            // delivery to copy-only, and truncated the transcript
                            // to whatever streamed so far. Instead, transition to
                            // finalizing exactly like a user Stop — flush what the
                            // server already has and deliver via the chosen mode.
                            tracing::warn!(
                                "dictation audio channel closed; finalizing gracefully"
                            );
                            finalizing = true;
                            emit_state(&app, DictationPhase::Processing, 0.0, mode);
                            ws_handle.finalize().await;
                            finalize_deadline
                                .as_mut()
                                .reset(tokio::time::Instant::now() + FINALIZE_TIMEOUT);
                        }
                    }
                    Some(Err(error)) => {
                        tracing::error!(%error, "dictation mic capture failed");
                        failed = true;
                        break;
                    }
                    None => {
                        tracing::warn!("dictation mic stream ended");
                        failed = true;
                        break;
                    }
                }
            }
            response = listen_stream.next() => {
                match response {
                    Some(Ok(response)) => {
                        let from_finalize =
                            handle_response(&app, response, mode, &mut typed_any, &mut transcript)
                                .await;
                        if from_finalize && finalizing {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        tracing::error!(?error, "dictation listen stream error");
                        failed = !finalizing;
                        break;
                    }
                    None => {
                        tracing::warn!(finalizing, "dictation listen stream closed");
                        failed = !finalizing;
                        break;
                    }
                }
            }
            _ = &mut finalize_deadline, if finalizing => {
                tracing::warn!("dictation finalize timed out waiting for the server flush");
                break;
            }
        }
    }

    drop(audio_tx);

    // Hand the accumulated raw transcript to the main window, which finishes
    // the job (cleanup per settings, batch delivery via `deliver_text`,
    // history entry). Emitted before the final state so listeners see the
    // session end in order: finished -> idle/error.
    if !transcript.is_empty() {
        let _ = DictationFinishedEvent {
            raw_text: transcript,
            mode,
            failed,
        }
        .emit(&app);
    }

    // Clear the slot (a normal stop already cleared it; error paths land here
    // with the slot still occupied).
    let state = app.state::<SessionState>();
    let _ = state.take();

    for phase in terminal_phases(failed) {
        emit_state(&app, *phase, 0.0, mode);
    }
}

/// The ordered orb phases emitted when a session ends. A clean finish
/// (`failed == false`) shows a one-shot [`DictationPhase::Success`] flourish
/// and then settles to [`DictationPhase::Idle`]; a failure goes straight to
/// [`DictationPhase::Error`]. Kept pure (no app handle) so the end-of-session
/// transition is unit-testable without a running session.
fn terminal_phases(failed: bool) -> &'static [DictationPhase] {
    if failed {
        &[DictationPhase::Error]
    } else {
        &[DictationPhase::Success, DictationPhase::Idle]
    }
}

/// Processes one server message: accumulates final transcript segments into
/// the session transcript and, in `type` mode, also injects them into the
/// focused app. Returns whether this was the post-Finalize flush marker.
async fn handle_response(
    app: &tauri::AppHandle<tauri::Wry>,
    response: StreamResponse,
    mode: DictationOutputMode,
    typed_any: &mut bool,
    transcript: &mut String,
) -> bool {
    let StreamResponse::TranscriptResponse {
        is_final,
        from_finalize,
        ..
    } = &response
    else {
        return false;
    };
    let (is_final, from_finalize) = (*is_final, *from_finalize);

    let text = response.text().unwrap_or_default().trim().to_string();

    if is_final && !text.is_empty() {
        if !transcript.is_empty() {
            transcript.push(' ');
        }
        transcript.push_str(&text);

        if mode == DictationOutputMode::Batch {
            let _ = DictationTranscriptEvent { text }.emit(app);
            return from_finalize;
        }

        // Join consecutive segments with a single space; the first segment is
        // typed as-is so dictation continues cleanly from the caret.
        let to_type = if *typed_any {
            format!(" {text}")
        } else {
            text.clone()
        };
        *typed_any = true;

        let inject_result =
            tauri::async_runtime::spawn_blocking(move || crate::inject::type_text(&to_type)).await;

        match inject_result {
            Ok(Ok(())) => {
                let _ = DictationTranscriptEvent { text }.emit(app);
            }
            Ok(Err(error)) => {
                tracing::error!(%error, "failed to inject dictated text");
            }
            Err(error) => {
                tracing::error!(%error, "text injection task panicked");
            }
        }
    }

    from_finalize
}

fn emit_state(
    app: &tauri::AppHandle<tauri::Wry>,
    phase: DictationPhase,
    amplitude: f32,
    mode: DictationOutputMode,
) {
    let _ = DictationStateEvent {
        phase,
        amplitude,
        mode,
    }
    .emit(app);
}

/// Broadcast one sample on the dense ~30 Hz amplitude channel.
fn emit_amplitude(app: &tauri::AppHandle<tauri::Wry>, amplitude: f32) {
    let _ = DictationAmplitudeEvent { amplitude }.emit(app);
}

/// Whether a throttled channel whose last emit was at `last` is due to fire at
/// `now`, given its minimum inter-emit `interval`. Pulled out so the emit
/// cadence is unit-testable with a simulated clock (see tests).
fn should_emit(now: std::time::Instant, last: std::time::Instant, interval: Duration) -> bool {
    now.duration_since(last) >= interval
}

/// RMS -> dB -> [0, 1], roughly matching the feel of the meeting pipeline's
/// amplitude without importing it: -50 dBFS maps to 0, 0 dBFS to 1.
fn normalized_amplitude(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let mean_square: f32 = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    let rms = mean_square.sqrt();
    if rms <= f32::EPSILON {
        return 0.0;
    }

    let db = 20.0 * rms.log10();
    ((db + 50.0) / 50.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        AMPLITUDE_FAST_INTERVAL, AMPLITUDE_INTERVAL, normalized_amplitude, resolve_soniqo_model,
        should_emit, terminal_phases,
    };
    use crate::events::DictationPhase;

    #[test]
    fn fast_amplitude_channel_runs_at_about_30hz() {
        // Simulate one second of mic frames arriving every 5 ms and count how
        // many pass each throttle, exactly as `run_session` does. The dense
        // channel should fire ~30x, the state channel ~10x, and the two must
        // advance independently (a state emit never resets the fast timer).
        let start = Instant::now();
        let mut last_fast = start - AMPLITUDE_FAST_INTERVAL;
        let mut last_state = start - AMPLITUDE_INTERVAL;
        let mut fast = 0;
        let mut state = 0;

        for i in 0..200u64 {
            let now = start + Duration::from_millis(i * 5);
            if should_emit(now, last_fast, AMPLITUDE_FAST_INTERVAL) {
                last_fast = now;
                fast += 1;
            }
            if should_emit(now, last_state, AMPLITUDE_INTERVAL) {
                last_state = now;
                state += 1;
            }
        }

        // 33 ms cadence over ~1 s, quantized to 5 ms frames: ~28-31 emits.
        assert!(
            (27..=32).contains(&fast),
            "fast channel emitted {fast} times"
        );
        // 100 ms cadence over ~1 s: ~10 emits.
        assert!(
            (10..=11).contains(&state),
            "state channel emitted {state} times"
        );
        // The fast channel is strictly the denser of the two.
        assert!(fast > state * 2, "fast={fast} should dwarf state={state}");
    }

    #[test]
    fn fast_interval_is_faster_than_the_state_interval() {
        assert!(AMPLITUDE_FAST_INTERVAL < AMPLITUDE_INTERVAL);
    }

    #[test]
    fn amplitude_always_stays_in_the_unit_range() {
        // Whatever the mic hands us - silence, full-scale, clipping beyond
        // full-scale, alternating - the emitted amplitude must be a clean
        // [0, 1] the frontend ref can trust without re-clamping.
        for level in [0.0f32, 1e-6, 0.001, 0.05, 0.5, 1.0, 5.0, -3.0] {
            let a = normalized_amplitude(&[level; 256]);
            assert!(
                (0.0..=1.0).contains(&a),
                "level {level} -> {a} out of [0,1]"
            );
        }
        let alternating: Vec<f32> = (0..256)
            .map(|i| if i % 2 == 0 { 0.8 } else { -0.8 })
            .collect();
        let a = normalized_amplitude(&alternating);
        assert!((0.0..=1.0).contains(&a), "alternating -> {a} out of [0,1]");
    }

    #[test]
    fn clean_finish_emits_a_success_flourish_then_settles_to_idle() {
        assert_eq!(
            terminal_phases(false),
            &[DictationPhase::Success, DictationPhase::Idle]
        );
    }

    #[test]
    fn failed_finish_emits_only_error() {
        assert_eq!(terminal_phases(true), &[DictationPhase::Error]);
    }

    #[test]
    fn amplitude_is_zero_for_silence() {
        assert_eq!(normalized_amplitude(&[]), 0.0);
        assert_eq!(normalized_amplitude(&[0.0; 128]), 0.0);
    }

    #[test]
    fn amplitude_is_one_for_full_scale() {
        assert_eq!(normalized_amplitude(&[1.0; 128]), 1.0);
    }

    #[test]
    fn amplitude_is_monotonic_in_level() {
        let quiet = normalized_amplitude(&[0.01; 128]);
        let medium = normalized_amplitude(&[0.1; 128]);
        let loud = normalized_amplitude(&[0.5; 128]);
        assert!(quiet < medium && medium < loud);
        assert!(quiet > 0.0 && loud < 1.0);
    }

    // Regression coverage for the macOS dictation-orb-never-starts bug: Soniqo
    // models (`soniqo-*`) must be recognized so `start()` routes them to
    // `LocalSoniqoLiveClient` instead of trying to open a WS connection to the
    // `soniqo://local` sentinel, which is not a URL at all.

    #[test]
    fn resolves_soniqo_model_from_local_sentinel_base_url() {
        let model = resolve_soniqo_model(
            hypr_transcribe_soniqo::LOCAL_BASE_URL,
            "soniqo-parakeet-streaming",
        )
        .unwrap();

        assert_eq!(
            model,
            Some(hypr_transcribe_soniqo::SoniqoModel::ParakeetStreaming)
        );
    }

    #[test]
    fn resolves_soniqo_model_from_loopback_http_base_url() {
        // `get_server_for_model` can also report a real loopback HTTP URL for
        // some serving paths; the model id alone still identifies Soniqo.
        let model =
            resolve_soniqo_model("http://127.0.0.1:50060/v1", "soniqo-parakeet-streaming").unwrap();

        assert_eq!(
            model,
            Some(hypr_transcribe_soniqo::SoniqoModel::ParakeetStreaming)
        );
    }

    #[test]
    fn non_soniqo_model_and_base_url_resolve_to_none() {
        let model = resolve_soniqo_model("http://127.0.0.1:5555", "QuantizedTiny").unwrap();
        assert_eq!(model, None);
    }

    #[test]
    fn invalid_model_on_the_local_sentinel_errors_instead_of_falling_through() {
        // A malformed model id paired with the local sentinel must not fall
        // through to the generic WS path (which would just hang trying to
        // connect to a non-URL) - it should surface as a clear error.
        let result = resolve_soniqo_model(hypr_transcribe_soniqo::LOCAL_BASE_URL, "not-a-model");
        assert!(result.is_err());
    }
}
