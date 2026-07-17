use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use hypr_local_model::LocalModel;
use hypr_model_downloader::ModelIntegrity;
use hypr_transcribe_core::SttEngine;
use serde_json::json;

use crate::state::AppState;

/// `GET /api/status` — always answers 200, even with no model installed
/// (see `docs/stt-server-design.md` §6). `loaded_model` is `null` until a
/// verified model exists on disk; download/activate land in Phase 2. GPU
/// `backends`/offload verification land in Phase 4 (`list_ggml_backends` is
/// release-build-only, see `crates/whisper-local/src/ggml.rs`; it returns an
/// empty list in this debug build and on this CPU image either way).
pub async fn status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let model = LocalModel::Whisper(state.config.model.clone());
    let integrity =
        hypr_model_downloader::verify_model(&model, &state.config.model_dir).unwrap_or_else(
            |error| ModelIntegrity::Corrupt(format!("integrity check failed: {error}")),
        );

    let loaded_model = match &integrity {
        ModelIntegrity::Verified | ModelIntegrity::PresentUnverified => Some(json!({
            "id": state.config.model.to_string(),
            "file": state.config.model.file_name(),
            "path": state.config.model_path().display().to_string(),
            "integrity": integrity,
        })),
        ModelIntegrity::NotInstalled | ModelIntegrity::Corrupt(_) => None,
    };

    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": <hypr_transcribe_whisper_local::LoadedWhisper as SttEngine>::arch(),
        "loadedModel": loaded_model,
        "modelIntegrity": integrity,
        "backends": hypr_whisper_local::list_ggml_backends(),
        "requireGpu": state.config.require_gpu,
        "uptimeSecs": state.start_time.elapsed().as_secs(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn status_answers_200_with_null_model_when_none_is_installed() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::Config {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let state = Arc::new(AppState::new(config));
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["loadedModel"].is_null());
        assert_eq!(json["modelIntegrity"]["state"], "notInstalled");
        assert_eq!(json["engine"], "whisper-local");
        assert!(json["backends"].is_array());
    }
}
