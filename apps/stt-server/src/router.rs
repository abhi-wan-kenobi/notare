use std::sync::Arc;

use axum::http::StatusCode;
use tower_http::cors::{self, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::admin;
use crate::state::AppState;

/// Build the full server router: `/health` + `/v1/listen` (batch + WS) from
/// the reused, engine-generic `hypr_transcribe_core::TranscribeService`,
/// merged with the `/api/*` admin surface. Matches
/// `docs/stt-server-design.md` §6 — the core router is served unchanged
/// ("exactly as the desktop's internal server does"); only the bind address
/// (`0.0.0.0` vs `LOCALHOST:0`) and the `/api/*` merge are new.
pub fn build_router(state: Arc<AppState>) -> axum::Router {
    let model_path = state.config.model_path();

    let core = hypr_transcribe_whisper_local::TranscribeService::builder()
        .model_path(model_path)
        .build()
        .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

    core.merge(admin::router(state))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
}

/// Permissive CORS, matching `crates/local-stt-server`'s
/// `cors_layer()` (LAN-only posture, no auth in Phase 1 — see
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
    /// once merged with `/api/*` behind our router + CORS/trace layers.
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
