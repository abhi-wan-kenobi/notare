use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::from_fn;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower::Service;
use tower_http::cors::{self, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::admin;
use crate::auth::require_bearer_token;
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
    let listen_auth_state = state.clone();
    let core = axum::Router::new()
        .route_service(
            hypr_transcribe_whisper_local::LISTEN_PATH,
            DynamicListen(state.clone()),
        )
        // Optional `NOTARE_STT_TOKEN` gate (no-op unless configured, see
        // `crate::auth`). Registered *before* `/health` is added below, so
        // `.layer()` here only ever wraps `/v1/listen` — liveness checks
        // must stay reachable even when a token is configured.
        .layer(from_fn(move |req: Request<Body>, next| {
            let state = listen_auth_state.clone();
            async move { require_bearer_token(state, req, next).await }
        }))
        .route(
            hypr_transcribe_whisper_local::HEALTH_PATH,
            get(|| async { "ok" }),
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

/// CORS allowlist (SEC-01 fix — this used to be `cors::Any`, letting *any*
/// website a user happened to have open in a tab POST/DELETE to this
/// LAN-bound server, e.g. `POST /api/models/{id}/activate` or
/// `DELETE /api/models/{id}`). Shares
/// `hypr_transcribe_core::is_allowed_origin` with the `/v1/listen`
/// WS-Origin check (SEC-02, `crates/transcribe-core/src/service/
/// streaming.rs`) so the two allowlists can never drift apart — see that
/// function's doc comment for exactly which origins are on it and why.
///
/// In practice neither of this server's real cross-origin callers actually
/// needs a CORS grant at all: the desktop's "Test connection" button
/// (`apps/desktop/src/settings/ai/stt/connection-test.ts`) calls
/// `@tauri-apps/plugin-http`'s `fetch`, which runs the HTTP request from the
/// Rust backend and is never subject to browser CORS; the real
/// transcription client (`crates/owhisper-client`) is plain `reqwest`/a
/// native WS client, also not CORS-bound; the embedded admin page
/// (`assets/index.html`) only ever calls same-origin, which browsers never
/// send a CORS preflight for. This allowlist is therefore defense-in-depth
/// against a *future* plain `window.fetch`/`new WebSocket(...)` from the
/// webview — it must never widen back to `cors::Any`.
///
/// Residual gap worth knowing: CORS does not stop a browser from *sending*
/// a same-site-cookie-free "simple" cross-origin `POST` with no custom
/// headers (only from letting the calling page *read* the response) — so a
/// malicious page could still blind-POST
/// `/api/models/{id}/{download,activate,cancel}` even with this allowlist
/// in place (though it can't read the result, and `DELETE` *is* blocked,
/// since non-simple methods always get a preflight the disallowed origin
/// will fail). `NOTARE_STT_TOKEN` (`crate::auth`) is the real mitigation for
/// that residual gap, not CORS — see the README's security section.
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _request_parts| {
            hypr_transcribe_core::is_allowed_origin(origin)
        }))
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

    /// Proves the merged router — Phase 3's embedded admin page at `GET /`
    /// plus Phase 2's *real* model-management mutations on `/api/models/*`
    /// — serves both from one `build_router` call with no route-table
    /// collision, and that the mutation route answers a real status code
    /// (`202`), not the Phase 1 `501 not_implemented` stub these two
    /// branches originally shipped against independently.
    #[tokio::test]
    async fn the_admin_page_and_real_model_mutations_share_one_router() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let index_response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(index_response.status(), StatusCode::OK);
        let content_type = index_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(content_type.starts_with("text/html"));

        let download_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/models/QuantizedTiny/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(download_response.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(download_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "downloading");
    }

    /// SEC-01: a disallowed origin must not get an
    /// `Access-Control-Allow-Origin` grant — this is what stops a malicious
    /// page's script from reading the response (the request itself may
    /// still reach the server for "simple" methods; see `cors_layer`'s doc
    /// comment on that residual gap).
    #[tokio::test]
    async fn cors_rejects_a_disallowed_origin() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header(axum::http::header::ORIGIN, "https://malicious-site.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none(),
            "a disallowed origin must not receive an ACAO grant"
        );
    }

    /// SEC-01: an allowlisted Tauri origin gets the ACAO grant mirrored
    /// back, matching how the desktop's webview would need it if it ever
    /// called this server via plain `window.fetch` instead of
    /// `@tauri-apps/plugin-http` (see `cors_layer`'s doc comment).
    #[tokio::test]
    async fn cors_allows_a_tauri_origin() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header(axum::http::header::ORIGIN, "tauri://localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("tauri://localhost")
        );
    }

    /// SEC-01: a dev-mode localhost origin at a non-default port (Vite,
    /// other dev tooling) also gets the grant.
    #[tokio::test]
    async fn cors_allows_a_localhost_dev_origin() {
        let (_dir, state) = state_with_no_model();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header(axum::http::header::ORIGIN, "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("http://localhost:5173")
        );
    }

    fn state_with_token(token: &str) -> (tempfile::TempDir, Arc<AppState>) {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            model_dir: dir.path().to_path_buf(),
            token: Some(token.to_string()),
            ..Default::default()
        };
        let state = Arc::new(AppState::new(config));
        (dir, state)
    }

    /// `NOTARE_STT_TOKEN`, once configured, gates `/v1/listen`...
    #[tokio::test]
    async fn listen_requires_the_token_once_configured() {
        let (_dir, state) = state_with_token("s3cr3t");
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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// ...but `/health` stays reachable with no token at all, so liveness
    /// checks never break just because an operator turned the gate on.
    #[tokio::test]
    async fn health_stays_open_even_with_a_token_configured() {
        let (_dir, state) = state_with_token("s3cr3t");
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
    }

    /// The correct bearer token gets through.
    #[tokio::test]
    async fn listen_accepts_the_correct_bearer_token() {
        let (_dir, state) = state_with_token("s3cr3t");
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/listen?channels=1&sample_rate=16000")
                    .header("content-type", "audio/wav")
                    .header(axum::http::header::AUTHORIZATION, "Bearer s3cr3t")
                    .body(Body::from(vec![0u8; 16]))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Not 401 — it gets past the auth gate and fails downstream for the
        // unrelated reason this fixture has no model installed (same
        // `model_load_failed` contract as
        // `v1_listen_with_no_model_returns_a_clear_json_error`).
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
