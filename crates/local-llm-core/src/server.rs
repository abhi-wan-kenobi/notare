#[cfg(feature = "llama")]
mod inner {
    use std::net::Ipv4Addr;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::sse::{Event, KeepAlive, Sse};
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use axum::{Json, Router};
    use hypr_local_llm_llama::{ChatMessage, FinishReason, GenerateRequest, LlamaLlmModel};
    use serde::{Deserialize, Serialize};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::ReceiverStream;
    use tower_http::cors::CorsLayer;

    use crate::Error;

    #[derive(Clone)]
    struct AppState {
        model: Arc<Mutex<LlamaLlmModel>>,
    }

    pub struct LlmServer {
        base_url: String,
        shutdown_tx: tokio::sync::watch::Sender<()>,
        exit_rx: tokio::sync::watch::Receiver<bool>,
        task: tokio::task::JoinHandle<()>,
    }

    impl LlmServer {
        /// Loads `file_path` (blocking — model loads are CPU-bound and can
        /// take several seconds) and starts an in-process, loopback-only,
        /// OpenAI-compatible `/v1/chat/completions` server. `name` labels
        /// the model in logs; the server itself always answers with
        /// whatever single model it loaded, regardless of the `model` field
        /// a caller sends (same "one model per server instance" shape the
        /// removed Cactus-backed implementation had).
        pub async fn start_with_model_path(
            name: String,
            file_path: impl AsRef<Path>,
        ) -> Result<Self, Error> {
            let file_path = file_path.as_ref().to_path_buf();
            if !file_path.exists() {
                return Err(Error::ModelNotDownloaded);
            }

            let load_path = file_path.clone();
            let model = tokio::task::spawn_blocking(move || LlamaLlmModel::load(&load_path))
                .await
                .map_err(|e| Error::Other(format!("model load task panicked: {e}")))?
                .map_err(|e| Error::Other(e.to_string()))?;

            let state = AppState {
                model: Arc::new(Mutex::new(model)),
            };

            let router = Router::new()
                .route("/v1/chat/completions", post(chat_completions))
                .layer(CorsLayer::permissive())
                .with_state(state);

            // Loopback-only, port 0 (OS-assigned) — never a routable
            // interface, same invariant the removed Cactus-backed server had.
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0u16))
                .await
                .map_err(Error::IoError)?;
            let addr = listener.local_addr().map_err(Error::IoError)?;
            let base_url = format!("http://{addr}/v1");

            let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());
            let (exit_tx, exit_rx) = tokio::sync::watch::channel(false);

            let server_task = tokio::spawn(async move {
                let _ = axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.changed().await;
                    })
                    .await;
            });

            let task = tokio::spawn(async move {
                if let Err(error) = server_task.await {
                    tracing::error!(error = %error, "local LLM server task crashed");
                }

                let _ = exit_tx.send(true);
            });

            tracing::info!(url = %base_url, model = %name, "local LLM server started");

            Ok(Self {
                base_url,
                shutdown_tx,
                exit_rx,
                task,
            })
        }

        pub fn url(&self) -> &str {
            &self.base_url
        }

        pub fn exit_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
            self.exit_rx.clone()
        }

        pub async fn stop(self) {
            let _ = self.shutdown_tx.send(());
            let _ = self.task.await;
            tracing::info!("local LLM server stopped");
        }
    }

    // ---- OpenAI-compatible wire types ----

    #[derive(Deserialize)]
    struct ChatCompletionRequest {
        #[serde(default)]
        model: Option<String>,
        messages: Vec<WireMessage>,
        #[serde(default)]
        max_tokens: Option<usize>,
        #[serde(default)]
        temperature: Option<f32>,
        #[serde(default)]
        stream: bool,
        #[serde(default)]
        response_format: Option<ResponseFormat>,
    }

    #[derive(Deserialize)]
    struct WireMessage {
        role: String,
        content: String,
    }

    #[derive(Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum ResponseFormat {
        JsonObject,
        JsonSchema { json_schema: JsonSchemaSpec },
    }

    #[derive(Deserialize)]
    struct JsonSchemaSpec {
        schema: serde_json::Value,
    }

    #[derive(Serialize)]
    struct ChatCompletionResponse {
        id: String,
        object: &'static str,
        created: u64,
        model: String,
        choices: Vec<Choice>,
        usage: Usage,
    }

    #[derive(Serialize)]
    struct Choice {
        index: u32,
        message: WireMessageOut,
        finish_reason: &'static str,
    }

    #[derive(Serialize)]
    struct WireMessageOut {
        role: &'static str,
        content: String,
    }

    #[derive(Serialize)]
    struct Usage {
        prompt_tokens: usize,
        completion_tokens: usize,
        total_tokens: usize,
    }

    #[derive(Serialize)]
    struct ChunkResponse {
        id: String,
        object: &'static str,
        created: u64,
        model: String,
        choices: Vec<ChunkChoice>,
    }

    #[derive(Serialize)]
    struct ChunkChoice {
        index: u32,
        delta: Delta,
        finish_reason: Option<&'static str>,
    }

    #[derive(Serialize, Default)]
    struct Delta {
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<&'static str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    }

    fn to_generate_request(req: &ChatCompletionRequest) -> GenerateRequest {
        let messages = req
            .messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        // `json_object` and `json_schema` both drive llama.cpp's `llguidance`
        // grammar sampler (Requirement 3): a bare `{"type": "object"}`
        // schema for `json_object`, the caller's own schema for
        // `json_schema`. Either way decoding is grammar-constrained, so the
        // emitted text is guaranteed schema-valid JSON rather than merely
        // likely to be.
        let json_schema = match &req.response_format {
            Some(ResponseFormat::JsonObject) => {
                Some(serde_json::json!({"type": "object"}).to_string())
            }
            Some(ResponseFormat::JsonSchema { json_schema }) => {
                Some(json_schema.schema.to_string())
            }
            None => None,
        };

        GenerateRequest {
            messages,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            json_schema,
        }
    }

    fn finish_reason_str(reason: FinishReason) -> &'static str {
        match reason {
            FinishReason::Stop => "stop",
            FinishReason::Length => "length",
        }
    }

    struct ApiError(StatusCode, String);

    impl IntoResponse for ApiError {
        fn into_response(self) -> Response {
            let body = serde_json::json!({ "error": { "message": self.1 } });
            (self.0, Json(body)).into_response()
        }
    }

    fn unix_now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn completion_id() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!(
            "chatcmpl-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }

    async fn chat_completions(
        State(state): State<AppState>,
        Json(req): Json<ChatCompletionRequest>,
    ) -> Response {
        if req.stream {
            stream_completion(state, req).into_response()
        } else {
            match complete(state, req).await {
                Ok(resp) => Json(resp).into_response(),
                Err(e) => e.into_response(),
            }
        }
    }

    async fn complete(
        state: AppState,
        req: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ApiError> {
        let model_name = req.model.clone().unwrap_or_else(|| "local".to_string());
        let generate_request = to_generate_request(&req);

        let model = state.model;
        let outcome = tokio::task::spawn_blocking(move || {
            let model = model
                .lock()
                .map_err(|_| "local LLM model mutex poisoned".to_string())?;
            model
                .generate(&generate_request, |_piece| true)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("generation task panicked: {e}"),
            )
        })?
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e))?;

        Ok(ChatCompletionResponse {
            id: completion_id(),
            object: "chat.completion",
            created: unix_now(),
            model: model_name,
            choices: vec![Choice {
                index: 0,
                message: WireMessageOut {
                    role: "assistant",
                    content: outcome.text,
                },
                finish_reason: finish_reason_str(outcome.finish_reason),
            }],
            usage: Usage {
                prompt_tokens: outcome.prompt_tokens,
                completion_tokens: outcome.completion_tokens,
                total_tokens: outcome.prompt_tokens + outcome.completion_tokens,
            },
        })
    }

    /// Chunk backlog before `blocking_send` (called from the dedicated
    /// `spawn_blocking` thread, so blocking here costs nothing on the tokio
    /// runtime) applies backpressure against a client reading slower than
    /// the model generates — an unbounded channel would instead buffer
    /// every pending chunk in memory with no limit.
    const STREAM_CHANNEL_CAPACITY: usize = 32;

    /// A stream error has no first-class OpenAI SSE shape; an `error` object
    /// in place of `choices` is what OpenAI's own server sends on a
    /// mid-stream failure, and what a spec-following client looks for
    /// instead of `choices`.
    fn error_event(message: String) -> Event {
        Event::default()
            .json_data(
                serde_json::json!({ "error": { "message": message, "type": "server_error" } }),
            )
            .unwrap_or_else(|_| Event::default())
    }

    fn stream_completion(
        state: AppState,
        req: ChatCompletionRequest,
    ) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
        let (tx, rx) = mpsc::channel::<Event>(STREAM_CHANNEL_CAPACITY);
        let model_name = req.model.clone().unwrap_or_else(|| "local".to_string());
        let generate_request = to_generate_request(&req);

        // Kept alive past the generator so a panic inside it can still be
        // reported on the wire — see the supervisor below.
        let panic_tx = tx.clone();

        let generator = tokio::task::spawn_blocking(move || {
            let id = completion_id();
            let created = unix_now();

            let chunk = |delta: Delta, finish_reason: Option<&'static str>| {
                let body = ChunkResponse {
                    id: id.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: model_name.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta,
                        finish_reason,
                    }],
                };
                Event::default()
                    .json_data(&body)
                    .unwrap_or_else(|_| Event::default())
            };

            let _ = tx.blocking_send(chunk(
                Delta {
                    role: Some("assistant"),
                    content: None,
                },
                None,
            ));

            let model = match state.model.lock() {
                Ok(m) => m,
                Err(_) => {
                    tracing::error!("local LLM model mutex poisoned");
                    let _ = tx.blocking_send(error_event(
                        "local LLM model is unavailable (poisoned lock)".to_string(),
                    ));
                    let _ = tx.blocking_send(Event::default().data("[DONE]"));
                    return;
                }
            };

            let result = model.generate(&generate_request, |piece| {
                tx.blocking_send(chunk(
                    Delta {
                        role: None,
                        content: Some(piece.to_string()),
                    },
                    None,
                ))
                .is_ok()
            });

            match result {
                Ok(outcome) => {
                    let _ = tx.blocking_send(chunk(
                        Delta::default(),
                        Some(finish_reason_str(outcome.finish_reason)),
                    ));
                }
                Err(e) => {
                    tracing::error!(error = %e, "local LLM streaming generation failed");
                    let _ = tx.blocking_send(error_event(e.to_string()));
                }
            }

            let _ = tx.blocking_send(Event::default().data("[DONE]"));
        });

        // `spawn_blocking` captures a panic in its `JoinHandle` rather than
        // unwinding the process. Dropping that handle would discard it: the
        // generator's `tx` is dropped by the unwind, the stream simply ends
        // with neither a `finish_reason` chunk nor `[DONE]`, and a client
        // waiting on a terminator sees a silent truncation it cannot
        // distinguish from a short answer. The model mutex is poisoned by
        // exactly this path — the poison branch above only exists because
        // panics here are considered reachable — so report it on the wire
        // instead of dropping it on the floor.
        tokio::spawn(async move {
            if let Err(error) = generator.await {
                tracing::error!(error = %error, "local LLM streaming generation panicked");
                let _ = panic_tx
                    .send(error_event(
                        "local LLM generation panicked mid-stream".to_string(),
                    ))
                    .await;
                let _ = panic_tx.send(Event::default().data("[DONE]")).await;
            }
        });

        let stream = ReceiverStream::new(rx).map(Ok);
        Sse::new(stream).keep_alive(KeepAlive::default())
    }
}

#[cfg(not(feature = "llama"))]
mod inner {
    use std::path::Path;

    use crate::Error;

    pub struct LlmServer {
        _private: (),
    }

    impl LlmServer {
        pub async fn start_with_model_path(
            _name: String,
            _file_path: impl AsRef<Path>,
        ) -> Result<Self, Error> {
            Err(Error::Other(
                "Local LLM is not enabled in this build".to_string(),
            ))
        }

        pub fn url(&self) -> &str {
            unreachable!()
        }

        pub fn exit_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
            unreachable!()
        }

        pub async fn stop(self) {}
    }
}

pub use inner::LlmServer;
