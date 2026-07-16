//! The dictation session (Windows/Linux): default microphone -> local STT
//! websocket (`/v1/listen`) -> final transcript segments injected into the
//! focused app.
//!
//! Deliberately lighter than the meeting capture pipeline
//! (`crates/listener-core`): mic-only, nothing written to disk, no note or
//! transcript rows created. It talks to the same internal whisper server the
//! meeting flow uses (the renderer passes the server's base URL and model,
//! resolved via the local-stt plugin).

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
    DictationOutputMode, DictationPhase, DictationStateEvent, DictationTranscriptEvent,
};
use crate::orb;

/// Matches the whisper server's expected input (`TARGET_SAMPLE_RATE`,
/// `crates/transcribe-core/src/audio.rs`).
const SAMPLE_RATE: u32 = 16_000;
/// Throttle for orb amplitude updates (same cadence as the meeting pipeline's
/// `AmplitudeEmitter`).
const AMPLITUDE_INTERVAL: Duration = Duration::from_millis(100);
/// How long to wait for the server to flush segments after a Finalize.
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(5);

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

    let (audio_tx, audio_rx) =
        tokio::sync::mpsc::channel::<MixedMessage<bytes::Bytes, ControlMessage>>(32);
    let outbound = tokio_stream::wrappers::ReceiverStream::new(audio_rx);

    let (listen_stream, ws_handle) = client
        .from_realtime_audio(outbound)
        .await
        .map_err(|e| Error::Session(format!("listen_ws_connect_failed: {e:?}")))?;

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();

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

    Ok(())
}

pub fn stop(app: &tauri::AppHandle<tauri::Wry>) {
    let state = app.state::<SessionState>();
    if let Some(session) = state.take() {
        // The session task observes the sender drop even if send fails.
        let _ = session.stop_tx.send(());
    }
}

async fn run_session(
    app: tauri::AppHandle<tauri::Wry>,
    mut mic: hypr_audio::CaptureStream,
    audio_tx: tokio::sync::mpsc::Sender<MixedMessage<bytes::Bytes, ControlMessage>>,
    listen_stream: impl futures_util::Stream<Item = Result<StreamResponse, hypr_ws_client::Error>>,
    ws_handle: impl owhisper_client::FinalizeHandle,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    mode: DictationOutputMode,
) {
    futures_util::pin_mut!(listen_stream);

    let mut finalizing = false;
    let mut failed = false;
    let mut typed_any = false;
    // Batch-paste mode: final segments accumulate here instead of being typed.
    let mut batch_buffer = String::new();
    let mut last_amplitude_at = std::time::Instant::now() - AMPLITUDE_INTERVAL;

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

                        if last_amplitude_at.elapsed() >= AMPLITUDE_INTERVAL {
                            last_amplitude_at = std::time::Instant::now();
                            emit_state(
                                &app,
                                DictationPhase::Listening,
                                normalized_amplitude(&samples),
                                mode,
                            );
                        }

                        let bytes = hypr_audio_utils::f32_to_i16_bytes(samples.iter().copied());
                        if audio_tx.send(MixedMessage::Audio(bytes)).await.is_err() {
                            tracing::warn!("dictation audio channel closed unexpectedly");
                            failed = true;
                            break;
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
                            handle_response(&app, response, mode, &mut typed_any, &mut batch_buffer)
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

    if mode == DictationOutputMode::BatchPaste {
        deliver_batch(&batch_buffer, failed).await;
    }

    // Clear the slot (a normal stop already cleared it; error paths land here
    // with the slot still occupied).
    let state = app.state::<SessionState>();
    let _ = state.take();

    if failed {
        emit_state(&app, DictationPhase::Error, 0.0, mode);
    } else {
        emit_state(&app, DictationPhase::Idle, 0.0, mode);
    }
}

/// Batch-paste delivery at the end of a session: clean the accumulated
/// transcript, copy it to the clipboard and paste it into the focused app.
/// The clipboard is intentionally NOT restored so the text stays available
/// for repeated pastes. When the session `failed`, only copy (no paste): the
/// dictated text is preserved without typing into whatever is focused.
async fn deliver_batch(batch_buffer: &str, failed: bool) {
    let cleaned = crate::clean::clean_transcript(batch_buffer);
    if cleaned.is_empty() {
        return;
    }

    let result = tauri::async_runtime::spawn_blocking(move || {
        if failed {
            crate::inject::copy_text(&cleaned)
        } else {
            crate::inject::paste_text(&cleaned)
        }
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::error!(%error, failed, "failed to deliver batch dictation transcript");
        }
        Err(error) => {
            tracing::error!(%error, "batch dictation delivery task panicked");
        }
    }
}

/// Processes one server message: injects final transcript segments into the
/// focused app (`type` mode) or accumulates them (`batch-paste` mode).
/// Returns whether this was the post-Finalize flush marker.
async fn handle_response(
    app: &tauri::AppHandle<tauri::Wry>,
    response: StreamResponse,
    mode: DictationOutputMode,
    typed_any: &mut bool,
    batch_buffer: &mut String,
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
        if mode == DictationOutputMode::BatchPaste {
            if !batch_buffer.is_empty() {
                batch_buffer.push(' ');
            }
            batch_buffer.push_str(&text);
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
    use super::normalized_amplitude;

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
}
