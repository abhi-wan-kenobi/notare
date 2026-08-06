use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::{
        FromRequestParts,
        ws::{Message, WebSocketUpgrade},
    },
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, Stream, StreamExt, stream::poll_fn};
use hypr_audio_chunking::{SpeechChunkExt, SpeechChunkingConfig};
use hypr_audio_interface::AsyncSource;
use hypr_model_manager::{ModelManager, ModelManagerBuilder};
use hypr_ws_utils::ConnectionManager;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, ListenParams};
use tokio::sync::mpsc;
use tower::Service;

use crate::TARGET_SAMPLE_RATE;
use crate::engine::{SttEngine, SttEngineSession};

use super::batch;
use super::message::{AudioExtract, IncomingMessage, process_incoming_message};
use super::response::{
    TranscriptKind, build_transcript_response, format_timestamp_now, send_ws, send_ws_best_effort,
};
use super::{
    build_metadata, build_session_with_languages, parse_listen_params, redemption_time,
    transcribe_chunk,
};

pub const LISTEN_PATH: &str = "/v1/listen";
pub const HEALTH_PATH: &str = "/health";

pub struct TranscribeService<E: SttEngine> {
    model_path: PathBuf,
    manager: ModelManager<E>,
    connection_manager: ConnectionManager,
}

impl<E: SttEngine> Clone for TranscribeService<E> {
    fn clone(&self) -> Self {
        Self {
            model_path: self.model_path.clone(),
            manager: self.manager.clone(),
            connection_manager: self.connection_manager.clone(),
        }
    }
}

/// Boxed future returned by a [`TranscribeService::prewarm_fn`] closure.
pub type PrewarmFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

impl<E: SttEngine> TranscribeService<E> {
    pub fn builder() -> TranscribeServiceBuilder<E> {
        TranscribeServiceBuilder::default()
    }

    /// Build a cheap, cloneable prewarm closure over this service's model
    /// manager. Calling it drives the exact same `manager.get(None)` path the
    /// `/v1/listen` upgrade uses (see `Service::call` above): it loads the
    /// default model if it was evicted and refreshes `last_activity` so the
    /// model-manager's 60s inactivity eviction won't fire while dictation is
    /// keeping it warm. It is a no-op-cheap `Arc::clone` when the model is
    /// already resident, and — unlike the upgrade path — never touches the
    /// `ConnectionManager`, so keeping a model warm can't cancel a live
    /// dictation/meeting session.
    pub fn prewarm_fn(&self) -> impl Fn() -> PrewarmFuture + Clone + Send + Sync + 'static {
        let manager = self.manager.clone();
        move || {
            let manager = manager.clone();
            Box::pin(async move {
                manager
                    .get(None)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
        }
    }

    pub fn into_router<F, Fut>(self, on_error: F) -> axum::Router
    where
        F: FnOnce(String) -> Fut + Clone + Send + Sync + 'static,
        Fut: std::future::Future<Output = (StatusCode, String)> + Send,
    {
        let service = axum::error_handling::HandleError::new(self, on_error);
        axum::Router::new()
            .route(HEALTH_PATH, axum::routing::get(|| async { "ok" }))
            .route_service(LISTEN_PATH, service)
    }
}

pub struct TranscribeServiceBuilder<E: SttEngine> {
    model_path: Option<PathBuf>,
    connection_manager: Option<ConnectionManager>,
    _engine: PhantomData<fn() -> E>,
}

impl<E: SttEngine> Default for TranscribeServiceBuilder<E> {
    fn default() -> Self {
        Self {
            model_path: None,
            connection_manager: None,
            _engine: PhantomData,
        }
    }
}

impl<E: SttEngine> TranscribeServiceBuilder<E> {
    pub fn model_path(mut self, model_path: PathBuf) -> Self {
        self.model_path = Some(model_path);
        self
    }

    pub fn build(self) -> TranscribeService<E> {
        let model_path = self
            .model_path
            .expect("TranscribeServiceBuilder requires model_path");
        let manager = ModelManagerBuilder::default()
            .register("default", &model_path)
            .default_model("default")
            .build();

        let warmup_manager = manager.clone();
        tokio::spawn(async move {
            match warmup_manager.get(None).await {
                Ok(_) => tracing::info!(engine = E::arch(), "stt_model_warmup_completed"),
                Err(error) => {
                    tracing::warn!(engine = E::arch(), error = %error, "stt_model_warmup_failed")
                }
            }
        });

        TranscribeService {
            model_path,
            manager,
            connection_manager: self.connection_manager.unwrap_or_default(),
        }
    }
}

impl<E: SttEngine> Service<Request<Body>> for TranscribeService<E> {
    type Response = Response;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let model_path = self.model_path.clone();
        let manager = self.manager.clone();
        let connection_manager = self.connection_manager.clone();

        Box::pin(async move {
            let is_ws = req
                .headers()
                .get("upgrade")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.eq_ignore_ascii_case("websocket"))
                .unwrap_or(false);

            let params = match parse_listen_params(req.uri().query().unwrap_or("")) {
                Ok(params) => params,
                Err(error) => {
                    return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                }
            };

            if is_ws {
                // SEC-02 (Cross-Site WebSocket Hijacking): reject a WS
                // handshake whose `Origin` header is present but not on the
                // shared allowlist (`crate::is_allowed_origin` — the exact
                // same list `apps/stt-server/src/router.rs::cors_layer` uses
                // for CORS, so the two checks can't drift apart).
                //
                // Policy for a *missing* `Origin` header: allow. Browsers
                // always send `Origin` on a cross-origin (and same-origin)
                // WebSocket handshake, so its absence means the caller isn't
                // a browser at all — e.g. the desktop's native
                // `owhisper-client` WS client, `curl`/smoke-test tooling, or
                // `apps/stt-server/src/probe.rs`'s self-request (which is a
                // batch POST anyway, not this branch). Treating "no Origin"
                // as trusted is what keeps this from breaking the existing
                // coruscant deployment's live desktop clients or the manual
                // `transcribe-whisper-local::examples::serve` smoke test —
                // none of them ever set an `Origin` header.
                if let Some(origin) = req.headers().get(axum::http::header::ORIGIN) {
                    if !crate::is_allowed_origin(origin) {
                        return Ok((StatusCode::FORBIDDEN, "invalid_origin").into_response());
                    }
                }

                let model = match manager.get(None).await {
                    Ok(model) => model,
                    Err(error) => {
                        tracing::error!(error = %error, "failed_to_load_model");
                        return Ok((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("failed to load model: {error}"),
                        )
                            .into_response());
                    }
                };

                let metadata = build_metadata::<E>(&model_path);
                let (mut parts, _body) = req.into_parts();
                let ws_upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                    Ok(ws) => ws,
                    Err(error) => {
                        return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                    }
                };

                let guard = connection_manager.acquire_connection();
                Ok(ws_upgrade
                    .on_upgrade(move |socket| async move {
                        handle_websocket(socket, params, metadata, guard, model, manager).await;
                    })
                    .into_response())
            } else {
                let content_type = req
                    .headers()
                    .get("content-type")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let accept = req
                    .headers()
                    .get("accept")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = match axum::body::to_bytes(req.into_body(), 100 * 1024 * 1024).await {
                    Ok(body) => body,
                    Err(error) => {
                        return Ok((StatusCode::BAD_REQUEST, error.to_string()).into_response());
                    }
                };

                if body.is_empty() {
                    return Ok((StatusCode::BAD_REQUEST, "request body is empty").into_response());
                }

                if accept.contains("text/event-stream") {
                    Ok(
                        batch::handle_batch_sse(
                            body,
                            &content_type,
                            &params,
                            &manager,
                            &model_path,
                        )
                        .await,
                    )
                } else {
                    Ok(
                        batch::handle_batch(body, &content_type, &params, &manager, &model_path)
                            .await,
                    )
                }
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopReason {
    End,
    Finalize,
}

/// Bounded send into the transcription pipeline. If the pipeline stops draining
/// (its output consumer wedged on a half-open socket during a network hiccup),
/// this mpsc backpressures indefinitely and freezes the session loop. Time it out
/// so the session ends with an error instead of hanging the client forever.
/// Returns `true` when the send FAILED (channel closed or timed out).
async fn pipeline_send_failed<T>(tx: &tokio::sync::mpsc::Sender<T>, item: T) -> bool {
    match tokio::time::timeout(crate::transport::WS_SEND_TIMEOUT, tx.send(item)).await {
        Ok(Ok(())) => false,
        Ok(Err(_)) => true,
        Err(_) => {
            tracing::warn!(
                timeout_secs = crate::transport::WS_SEND_TIMEOUT.as_secs(),
                "audio_pipeline_send_timed_out: transcription pipeline not draining, ending session"
            );
            true
        }
    }
}

async fn handle_websocket<E: SttEngine>(
    socket: axum::extract::ws::WebSocket,
    params: ListenParams,
    metadata: owhisper_interface::stream::Metadata,
    guard: hypr_ws_utils::ConnectionGuard,
    model: Arc<E>,
    manager: ModelManager<E>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let total_channels = (params.channels as usize).max(1);
    let redemption_time = redemption_time(&params);
    let languages: Vec<hypr_language::Language> = params.languages.clone();
    let provider = E::arch();
    let dictation = super::is_dictation(&params);
    match build_transcription_streams(
        total_channels,
        model.as_ref(),
        &languages,
        redemption_time,
        dictation,
    ) {
        Ok((audio_txs, mut stream)) => {
            let mut audio_txs = audio_txs;
            // Register this session for the live dashboard; the guard records it
            // as ended on any of the loop's exit paths.
            let _activity =
                crate::activity::begin_guarded(metadata.request_id.clone(), provider.to_string());
            let mut stop_reason = None;
            let mut receiving_input = true;
            let mut channel_audio_durations = vec![0.0_f64; total_channels];
            let mut mono_mixdown = hypr_audio_utils::MonoMixdown::new(TARGET_SAMPLE_RATE);
            let mut stream_closed = false;

            // Keep the WebSocket warm through intermediary proxies (e.g.
            // Tailscale Serve) and stateful NAT. A long transcription whose
            // outbound segments are sparse — the server can spend many seconds
            // decoding a single chunk with nothing flowing on the wire — can
            // otherwise have its connection silently idled out, freezing the
            // client mid-stream with no error. A ~15s WS ping sits well under
            // typical proxy/NAT idle windows and lets a broken path surface
            // quickly instead of hanging.
            let mut keepalive = tokio::time::interval_at(
                tokio::time::Instant::now() + std::time::Duration::from_secs(15),
                std::time::Duration::from_secs(15),
            );
            keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            while !stream_closed {
                tokio::select! {
                    _ = guard.cancelled() => {
                        tracing::info!("websocket_cancelled_by_new_connection");
                        break;
                    }
                    _ = keepalive.tick() => {
                        // Best-effort: a failed OR hung send means the peer/path
                        // is gone, so end the loop rather than spinning on (or
                        // blocking forever against) a dead socket.
                        match tokio::time::timeout(
                            crate::transport::WS_SEND_TIMEOUT,
                            ws_sender.send(Message::Ping(Default::default())),
                        )
                        .await
                        {
                            Ok(Ok(())) => {}
                            _ => {
                                tracing::info!("websocket_keepalive_ping_failed_peer_gone");
                                break;
                            }
                        }
                    }
                    item = stream.next() => {
                        match item {
                            Some(Ok((channel_idx, segment))) => {
                                // Live-dashboard progress: furthest audio position
                                // the engine has produced output for (a flat line
                                // on the graph = a stall).
                                crate::activity::registry()
                                    .progress(segment.start + segment.duration);
                                let channel_index = vec![channel_idx as i32, total_channels as i32];
                                let channel = vec![channel_idx as u8];
                                // Segments are always sent as confirmed; a single
                                // empty `from_finalize` marker is sent once the
                                // pipeline has fully drained (see below). Marking
                                // every drained segment as finalized would make
                                // clients stop reading after the first one and
                                // drop the rest of the tail.
                                let transcript_kind = TranscriptKind::Confirmed;

                                if !send_ws(&mut ws_sender, &StreamResponse::SpeechStartedResponse {
                                    channel: channel.clone(),
                                    timestamp: segment.start,
                                }).await {
                                    tracing::warn!("stream_ended: speech_started send failed (peer gone)");
                                    break;
                                }

                                if !send_ws(
                                    &mut ws_sender,
                                    &build_transcript_response(&segment, transcript_kind, &metadata, &channel_index),
                                ).await {
                                    tracing::warn!("stream_ended: transcript send failed (peer gone)");
                                    break;
                                }

                                if !send_ws(&mut ws_sender, &StreamResponse::UtteranceEndResponse {
                                    channel,
                                    last_word_end: segment.start + segment.duration,
                                }).await {
                                    tracing::warn!("stream_ended: utterance_end send failed (peer gone)");
                                    break;
                                }
                            }
                            Some(Err(error)) => {
                                // D3: this is the path that must never die silently.
                                // A transcription/engine failure (incl. an inference
                                // panic surfaced through the spawn_blocking join) is
                                // logged, reported to the client, then followed by a
                                // graceful WS close (via `ws_sender.close()` below).
                                tracing::error!(error = %error, provider, "transcription_stream_error_ending_session");
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: error.to_string(),
                                        provider: provider.to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                            None => {
                                stream_closed = true;
                            }
                        }
                    }
                    message = ws_receiver.next(), if receiving_input => {
                        manager.keep_alive().await;

                        let Some(message) = message else {
                            receiving_input = false;
                            stop_reason.get_or_insert(StopReason::End);
                            audio_txs.clear();
                            continue;
                        };

                        let message = match message {
                            Ok(message) => message,
                            Err(error) => {
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: format!("websocket receive error: {error}"),
                                        provider: provider.to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                        };

                        match process_incoming_message(&message, params.channels.max(1)) {
                            Ok(IncomingMessage::Audio(AudioExtract::Mono(samples))) => {
                                if samples.is_empty() {
                                    continue;
                                }
                                channel_audio_durations[0] += samples.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                if pipeline_send_failed(&audio_txs[0], samples).await {
                                    send_ws_best_effort(
                                        &mut ws_sender,
                                        &StreamResponse::ErrorResponse {
                                            error_code: None,
                                            error_message: "audio pipeline closed unexpectedly".to_string(),
                                            provider: provider.to_string(),
                                        },
                                    )
                                    .await;
                                    break;
                                }
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::Dual { ch0, ch1 })) => {
                                if total_channels >= 2 {
                                    channel_audio_durations[0] += ch0.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    channel_audio_durations[1] += ch1.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    if pipeline_send_failed(&audio_txs[0], ch0).await || pipeline_send_failed(&audio_txs[1], ch1).await {
                                        send_ws_best_effort(
                                            &mut ws_sender,
                                            &StreamResponse::ErrorResponse {
                                                error_code: None,
                                                error_message: "audio pipeline closed unexpectedly".to_string(),
                                                provider: provider.to_string(),
                                            },
                                        )
                                        .await;
                                        break;
                                    }
                                } else {
                                    let mixed = mono_mixdown.mix(&ch0, &ch1);
                                    channel_audio_durations[0] += mixed.len() as f64 / TARGET_SAMPLE_RATE as f64;
                                    if !mixed.is_empty() && pipeline_send_failed(&audio_txs[0], mixed).await {
                                        send_ws_best_effort(
                                            &mut ws_sender,
                                            &StreamResponse::ErrorResponse {
                                                error_code: None,
                                                error_message: "audio pipeline closed unexpectedly".to_string(),
                                                provider: provider.to_string(),
                                            },
                                        )
                                        .await;
                                        break;
                                    }
                                }
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::End)) => {
                                receiving_input = false;
                                stop_reason.get_or_insert(StopReason::End);
                                audio_txs.clear();
                            }
                            Ok(IncomingMessage::Audio(AudioExtract::Empty)) => {}
                            Ok(IncomingMessage::Control(ControlMessage::KeepAlive)) => {}
                            Ok(IncomingMessage::Control(ControlMessage::Finalize)) => {
                                receiving_input = false;
                                stop_reason = Some(StopReason::Finalize);
                                audio_txs.clear();
                            }
                            Ok(IncomingMessage::Control(ControlMessage::CloseStream)) => {
                                receiving_input = false;
                                stop_reason.get_or_insert(StopReason::End);
                                audio_txs.clear();
                            }
                            Err(error) => {
                                send_ws_best_effort(
                                    &mut ws_sender,
                                    &StreamResponse::ErrorResponse {
                                        error_code: None,
                                        error_message: error.to_string(),
                                        provider: provider.to_string(),
                                    },
                                )
                                .await;
                                break;
                            }
                        }
                    }
                }
            }

            if stream_closed {
                if stop_reason == Some(StopReason::Finalize) {
                    // Empty flush marker: tells the client that every segment
                    // produced before the Finalize control message has been
                    // delivered. Clients count `from_finalize` responses to
                    // know when the post-finalize drain is complete.
                    let marker_segment = crate::service::Segment {
                        text: String::new(),
                        start: 0.0,
                        duration: 0.0,
                        confidence: 0.0,
                        language: None,
                    };
                    send_ws_best_effort(
                        &mut ws_sender,
                        &build_transcript_response(
                            &marker_segment,
                            TranscriptKind::Finalized,
                            &metadata,
                            &[0, total_channels as i32],
                        ),
                    )
                    .await;
                }

                let total_duration = channel_audio_durations.into_iter().fold(0.0_f64, f64::max);
                send_ws_best_effort(
                    &mut ws_sender,
                    &StreamResponse::TerminalResponse {
                        request_id: metadata.request_id.clone(),
                        created: format_timestamp_now(),
                        duration: total_duration,
                        channels: total_channels as u32,
                    },
                )
                .await;
            }

            let _ = ws_sender.close().await;
        }
        Err(error) => {
            send_ws_best_effort(
                &mut ws_sender,
                &StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: error.to_string(),
                    provider: provider.to_string(),
                },
            )
            .await;
            let _ = ws_sender.close().await;
        }
    }
}

type TranscriptionStream =
    Pin<Box<dyn Stream<Item = Result<(usize, crate::service::Segment), crate::Error>> + Send>>;

#[allow(clippy::type_complexity)]
fn build_transcription_streams<E: SttEngine>(
    total_channels: usize,
    engine: &E,
    languages: &[hypr_language::Language],
    redemption_time: std::time::Duration,
    dictation: bool,
) -> Result<
    (
        Vec<mpsc::Sender<Vec<f32>>>,
        futures_util::stream::SelectAll<TranscriptionStream>,
    ),
    crate::Error,
> {
    let mut audio_txs = Vec::with_capacity(total_channels);
    let mut streams = futures_util::stream::SelectAll::new();

    // Dictation force-cuts long pauseless utterances into small chunks; the
    // meeting/`speech` profile is left untouched (chunks grow to a natural
    // pause). See `hypr_audio_chunking::SpeechChunkingConfig::dictation`.
    let chunking_config = if dictation {
        SpeechChunkingConfig::dictation(redemption_time)
    } else {
        SpeechChunkingConfig::speech(redemption_time)
    };

    for channel_idx in 0..total_channels {
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>(8);
        audio_txs.push(audio_tx);

        let session = build_session_with_languages(engine, languages.to_vec())?;
        // Hard cap on what any single `transcribe` call receives: the engine's
        // own limit (Parakeet lowers it to survive Windows DirectML — D3),
        // never above the universal `MAX_CHUNK_SAMPLES` ceiling.
        let engine_max_samples = session.max_samples_per_call();
        let max_samples = engine_max_samples.min(crate::audio::MAX_CHUNK_SAMPLES);

        // D3 field-bug instrumentation (WS-0, 2026-08-06): the Windows dictation
        // stall keeps recurring, and the two silent regressions are "the
        // dictation VAD profile was not applied" and "the engine cap did not
        // reach this stream". Emit both, once per session (channel 0), so a
        // single user-supplied log proves which profile + cap were actually in
        // force — no guessing. Pairs with `parakeet_execution_provider_active`
        // (which EP is live) and the client's `dictation_session_end`.
        if channel_idx == 0 {
            tracing::info!(
                profile = if dictation { "dictation" } else { "meeting" },
                engine = E::arch(),
                redemption_ms = redemption_time.as_millis() as u64,
                engine_max_samples,
                applied_max_samples = max_samples,
                ceiling_samples = crate::audio::MAX_CHUNK_SAMPLES,
                "transcription_streams_built"
            );
        }

        let chunk_stream = ChannelAudioSource::new(audio_rx).speech_chunks(chunking_config.clone());
        let stream: TranscriptionStream = Box::pin(TranscribeChannelStream::new(
            channel_idx,
            chunk_stream,
            session,
            max_samples,
        ));
        streams.push(stream);
    }

    Ok((audio_txs, streams))
}

struct ChannelAudioSource {
    receiver: mpsc::Receiver<Vec<f32>>,
    buffered: VecDeque<f32>,
}

impl ChannelAudioSource {
    fn new(receiver: mpsc::Receiver<Vec<f32>>) -> Self {
        Self {
            receiver,
            buffered: VecDeque::new(),
        }
    }
}

impl AsyncSource for ChannelAudioSource {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        poll_fn(move |cx| {
            loop {
                if let Some(sample) = self.buffered.pop_front() {
                    return Poll::Ready(Some(sample));
                }

                match self.receiver.poll_recv(cx) {
                    Poll::Ready(Some(chunk)) => {
                        self.buffered.extend(chunk);
                        continue;
                    }
                    Poll::Ready(None) => return Poll::Ready(None),
                    Poll::Pending => return Poll::Pending,
                }
            }
        })
    }

    fn sample_rate(&self) -> u32 {
        TARGET_SAMPLE_RATE
    }
}

/// Result of one off-thread decode: the session is returned so it can be
/// reused (`None` if the inference panicked and the session was dropped).
type DecodeOutcome<Sess> = (
    Option<Sess>,
    Result<Vec<crate::service::Segment>, crate::Error>,
);

struct TranscribeChannelStream<S, Sess> {
    channel_idx: usize,
    chunk_stream: S,
    /// Present while idle; `take`n for the duration of an off-thread decode.
    session: Option<Sess>,
    /// Engine inference runs on the blocking pool so a multi-second decode
    /// never stalls the connection's `select!` loop (starving keepalives /
    /// audio backpressure) and a native inference panic is caught at the join
    /// boundary instead of aborting the process (D3).
    inflight: Option<tokio::task::JoinHandle<DecodeOutcome<Sess>>>,
    /// Windows still to decode from the current VAD chunk (cap-split, in order).
    windows: VecDeque<(Vec<f32>, f64)>,
    pending: VecDeque<crate::service::Segment>,
    /// Hard ceiling on samples per `transcribe` call (engine cap ∧ universal).
    max_samples: usize,
    /// Set once the upstream chunk stream has ended, so it is never polled again
    /// after completion (which many streams treat as a contract violation).
    input_done: bool,
}

impl<S, Sess> TranscribeChannelStream<S, Sess> {
    fn new(channel_idx: usize, chunk_stream: S, session: Sess, max_samples: usize) -> Self {
        Self {
            channel_idx,
            chunk_stream,
            session: Some(session),
            inflight: None,
            windows: VecDeque::new(),
            pending: VecDeque::new(),
            max_samples: max_samples.max(1),
            input_done: false,
        }
    }
}

impl<S, Sess> Stream for TranscribeChannelStream<S, Sess>
where
    S: Stream<Item = Result<hypr_audio_chunking::AudioChunk, hypr_audio_chunking::Error>> + Unpin,
    Sess: SttEngineSession + Unpin,
{
    type Item = Result<(usize, crate::service::Segment), crate::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(segment) = this.pending.pop_front() {
                return Poll::Ready(Some(Ok((this.channel_idx, segment))));
            }

            // 1. Drive an in-flight off-thread decode to completion.
            if let Some(handle) = this.inflight.as_mut() {
                match Pin::new(handle).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(join_result) => {
                        this.inflight = None;
                        match join_result {
                            Ok((session, decode_result)) => {
                                this.session = session;
                                match decode_result {
                                    Ok(segments) => this.pending.extend(segments),
                                    Err(error) => return Poll::Ready(Some(Err(error))),
                                }
                            }
                            Err(join_error) => {
                                // The blocking task panicked or was cancelled;
                                // the connection loop logs + closes gracefully.
                                return Poll::Ready(Some(Err(crate::Error::Engine(
                                    crate::EngineError::new(format!(
                                        "inference task failed: {join_error}"
                                    )),
                                ))));
                            }
                        }
                        continue;
                    }
                }
            }

            // 2. Start the next queued window on the blocking pool.
            if let Some((window, start_sec)) = this.windows.pop_front() {
                let Some(mut session) = this.session.take() else {
                    return Poll::Ready(Some(Err(crate::Error::Engine(crate::EngineError::new(
                        "engine session unavailable after a prior inference failure",
                    )))));
                };
                let handle = tokio::task::spawn_blocking(move || {
                    // catch_unwind converts a Rust-level inference panic into an
                    // error (with panic=unwind, the workspace default) instead
                    // of unwinding across the FFI boundary. A true native abort
                    // (std::abort/segfault) still can't be caught in-process —
                    // that is what the pre-engine sample cap (Fix B) guards.
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        transcribe_chunk(&mut session, &window, start_sec)
                    }));
                    match outcome {
                        Ok(result) => (Some(session), result),
                        Err(_) => (
                            None,
                            Err(crate::Error::Engine(crate::EngineError::new(
                                "engine panicked during inference",
                            ))),
                        ),
                    }
                });
                this.inflight = Some(handle);
                continue;
            }

            // 3. Reaching here, pending/windows/inflight are all empty. If the
            // input has ended too, the stream is done.
            if this.input_done {
                return Poll::Ready(None);
            }

            // Pull the next VAD chunk and cap-split it into decode windows.
            match Pin::new(&mut this.chunk_stream).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    // Cap each VAD chunk before the engine sees it. Voxtral/
                    // libmtmd's fixed 30s window silently truncates a longer
                    // buffer; Parakeet lowers the cap further to survive Windows
                    // DirectML (D3). Mirrors the batch path's windowing in
                    // `crate::audio`.
                    for (index, window) in chunk.samples.chunks(this.max_samples).enumerate() {
                        let sample_start = chunk.sample_start + index * this.max_samples;
                        let start_sec = sample_start as f64 / TARGET_SAMPLE_RATE as f64;
                        this.windows.push_back((window.to_vec(), start_sec));
                    }
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error.into()))),
                Poll::Ready(None) => this.input_done = true,
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineError, EngineSegment, SttEngineSession};
    use crate::service::mock::MockEngine;
    use std::sync::{Arc, Mutex};

    #[test]
    fn health_and_listen_paths_are_stable() {
        assert_eq!(HEALTH_PATH, "/health");
        assert_eq!(LISTEN_PATH, "/v1/listen");
    }

    /// Records the sample count of every `transcribe` call so a test can assert
    /// the cap-split handed the engine correctly-bounded windows.
    struct CountingSession {
        calls: Arc<Mutex<Vec<usize>>>,
    }

    impl SttEngineSession for CountingSession {
        fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
            self.calls.lock().unwrap().push(samples.len());
            Ok(vec![EngineSegment {
                text: "x".to_string(),
                start: 0.0,
                end: samples.len() as f64 / TARGET_SAMPLE_RATE as f64,
                confidence: 1.0,
                language: None,
            }])
        }
    }

    /// Panics inside inference — stands in for a native engine that dies on a
    /// pathological buffer (D3). `spawn_blocking` + `catch_unwind` must turn
    /// this into a stream error, not a process abort.
    struct PanicSession;

    impl SttEngineSession for PanicSession {
        fn transcribe(&mut self, _samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
            panic!("simulated native inference crash");
        }
    }

    fn chunk_of(len: usize) -> hypr_audio_chunking::AudioChunk {
        hypr_audio_chunking::AudioChunk {
            samples: vec![0.1; len],
            sample_start: 0,
            sample_end: len,
        }
    }

    /// Fix B: a VAD chunk larger than the engine cap is split so no single
    /// `transcribe` call ever exceeds it — for any engine, not just Voxtral.
    #[tokio::test]
    async fn oversized_chunk_is_split_at_the_engine_cap_before_transcribe() {
        let cap = 1000usize;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session = CountingSession {
            calls: calls.clone(),
        };
        // 2500 samples with a 1000 cap => windows of 1000, 1000, 500.
        let chunks = futures_util::stream::iter(vec![Ok(chunk_of(2500))]);
        let stream = TranscribeChannelStream::new(0, chunks, session, cap);

        let out: Vec<_> = stream.collect().await;
        assert!(out.iter().all(|item| item.is_ok()));
        assert_eq!(&*calls.lock().unwrap(), &[1000, 1000, 500]);
    }

    /// Fix C: an inference panic surfaces as a stream error (caught at the
    /// blocking-join boundary) instead of unwinding across FFI / aborting the
    /// process — the connection loop then logs it and closes gracefully.
    #[tokio::test]
    async fn inference_panic_becomes_a_stream_error_not_a_process_abort() {
        // Silence the default panic hook's backtrace noise for the caught panic.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let chunks = futures_util::stream::iter(vec![Ok(chunk_of(1600))]);
        let stream = TranscribeChannelStream::new(0, chunks, PanicSession, 16_000);
        let out: Vec<_> = stream.collect().await;

        std::panic::set_hook(prev);

        assert_eq!(out.len(), 1, "expected exactly one (error) item");
        assert!(
            matches!(out[0], Err(crate::Error::Engine(_))),
            "panic must surface as an engine error"
        );
    }

    /// `MockEngine::load` ignores the path entirely, so this never touches
    /// disk — same trick the rest of this crate's mock-backed tests use.
    fn mock_service() -> TranscribeService<MockEngine> {
        TranscribeService::builder()
            .model_path(std::path::PathBuf::from("/nonexistent/mock.bin"))
            .build()
    }

    fn ws_upgrade_request(origin: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/v1/listen?channels=1&sample_rate=16000")
            .header("upgrade", "websocket");
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        builder.body(Body::empty()).unwrap()
    }

    /// SEC-02: a WS upgrade whose `Origin` is present but not on the shared
    /// allowlist must be rejected before any WS-specific handshake parsing
    /// (or model loading) happens at all — proven by getting `403` even
    /// though this request has none of the `sec-websocket-*` headers a real
    /// handshake would need, and even though `mock_service`'s model path
    /// does not exist on disk.
    #[tokio::test]
    async fn ws_upgrade_rejects_a_disallowed_origin() {
        let mut service = mock_service();
        let request = ws_upgrade_request(Some("https://malicious-site.com"));

        let response = service.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    /// A missing `Origin` header (native, non-browser clients — the
    /// desktop's WS client, curl, `probe.rs`) must pass the origin gate.
    /// This request is built directly (not through a real hyper
    /// connection), so it carries no `OnUpgrade` extension and
    /// `WebSocketUpgrade`'s extractor always rejects it with `500` a step
    /// later regardless of headers — the point of this test is only that it
    /// is a `500` (rejected further downstream, for an unrelated protocol
    /// reason) and specifically **not** the `403` the origin gate itself
    /// would produce.
    #[tokio::test]
    async fn ws_upgrade_allows_a_missing_origin() {
        let mut service = mock_service();
        let request = ws_upgrade_request(None);

        let response = service.call(request).await.unwrap();
        assert_ne!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// Same reasoning as above, for an explicitly allowlisted origin.
    #[tokio::test]
    async fn ws_upgrade_allows_a_tauri_origin() {
        let mut service = mock_service();
        let request = ws_upgrade_request(Some("tauri://localhost"));

        let response = service.call(request).await.unwrap();
        assert_ne!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
