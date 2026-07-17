use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower::Service;
use tower_http::cors::{self, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::admin;
use crate::state::AppState;

/// Build the full server router: `/health` (static) + `/v1/listen` (dynamic,
/// see [`DynamicListen`]) from the reused, engine-generic
/// `hypr_transcribe_core::TranscribeService`, merged with the `/api/*` admin
/// surface. Matches `docs/stt-server-design.md` §6 — the wire contract for
/// `/health`/`/v1/listen` is unchanged from Phase 1; only *which*
/// `TranscribeService` instance backs `/v1/listen` becomes swappable, so
/// `POST /api/models/{id}/activate` (Phase 2) can take effect without a
/// restart.
pub fn build_router(state: Arc<AppState>) -> axum::Router {
    let core = axum::Router::new()
        .route(
            hypr_transcribe_whisper_local::HEALTH_PATH,
            get(|| async { "ok" }),
        )
        .route_service(
            hypr_transcribe_whisper_local::LISTEN_PATH,
            DynamicListen(state.clone()),
        );

    core.merge(admin::router(state))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
}

/// Forwards `/v1/listen` to whichever `TranscribeService` is currently
/// active (`AppState::active`), so an `activate` call takes effect on the
/// very next request. This replaces Phase 1's "one `TranscribeService` built
/// once at startup, turned into a router via `into_router`" with "read the
/// current one under a lock, then dispatch it exactly as
/// `TranscribeService::into_router`'s `HandleError` wrapper did" — same
/// request/response contract, same error mapping
/// (`(StatusCode::INTERNAL_SERVER_ERROR, err)`), just with the target
/// swappable. `TranscribeService::call` is cheap to invoke on a clone: its
/// `Clone` impl only clones a `PathBuf` + `Arc`-backed `ModelManager` +
/// `ConnectionManager`, none of which are mutated by `poll_ready`/`call`
/// (see `crates/transcribe-core/src/service/streaming.rs`).
#[derive(Clone)]
struct DynamicListen(Arc<AppState>);

impl Service<Request<Body>> for DynamicListen {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let state = self.0.clone();
        Box::pin(async move {
            let mut core = state.active.read().await.core.clone();
            match core.call(req).await {
                Ok(response) => Ok(response),
                // Dead in practice today (every error path inside
                // `TranscribeService::call` already converts to a JSON
                // `Response` instead of returning `Err`), kept only so this
                // wrapper's error handling matches Phase 1's `into_router`
                // `on_error` closure exactly instead of silently dropping a
                // hypothetical future error variant.
                Err(error) => Ok((StatusCode::INTERNAL_SERVER_ERROR, error).into_response()),
            }
        })
    }
}

/// Permissive CORS, matching `crates/local-stt-server`'s
/// `cors_layer()` (LAN-only posture, no auth in Phase 1/2 — see
/// `docs/stt-server-design.md` §10).
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    /// Returns the backing `TempDir` alongside the state so callers keep it
    /// alive for the lifetime of the test (it deletes on drop).
    fn state_with_no_model() -> (tempfile::TempDir, Arc<AppState>) {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let state = Arc::new(AppState::new(config));
        (dir, state)
    }

    #[tokio::test]
    async fn health_answers_ok_even_with_no_model_installed() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"ok");
    }

    /// `/v1/listen` batch reuses `hypr_transcribe_core`'s own
    /// `model_load_failed` JSON error unchanged (see
    /// `crates/transcribe-whisper-local/src/lib.rs` tests for the upstream
    /// contract this mirrors) — this test only proves it is still reachable
    /// once merged with `/api/*` behind the dynamic dispatcher + CORS/trace
    /// layers.
    #[tokio::test]
    async fn v1_listen_with_no_model_returns_a_clear_json_error() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/listen?channels=1&sample_rate=16000")
                    .header("content-type", "audio/wav")
                    .body(Body::from(vec![0u8; 16]))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "model_load_failed");
    }
}
