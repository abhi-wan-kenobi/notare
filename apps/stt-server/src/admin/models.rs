use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use hypr_local_model::{LocalModel, WhisperModel};
use hypr_transcribe_core::json_error_response;
use serde_json::json;

use crate::state::AppState;

/// The whisper.cpp catalog this server can ever serve (Phase 1 is
/// whisper-only, see `docs/stt-server-design.md` §2). Mirrors the `ALL`
/// fixture in `crates/whisper-local-model/src/lib.rs` tests; there is no
/// `WhisperModel::all()` in that crate today.
const CATALOG: &[WhisperModel] = &[
    WhisperModel::QuantizedTiny,
    WhisperModel::QuantizedTinyEn,
    WhisperModel::QuantizedBase,
    WhisperModel::QuantizedBaseEn,
    WhisperModel::QuantizedSmall,
    WhisperModel::QuantizedSmallEn,
    WhisperModel::QuantizedLargeTurbo,
];

/// `GET /api/models` — read-only catalog + on-disk integrity per model.
/// Download/delete/activate are Phase 2 (`model-downloader` +
/// `ModelManager`); this only reuses `verify_model` against whatever is
/// already on disk.
pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let models: Vec<_> = CATALOG
        .iter()
        .map(|model| {
            let integrity = hypr_model_downloader::verify_model(
                &LocalModel::Whisper(model.clone()),
                &state.config.model_dir,
            )
            .unwrap_or(hypr_model_downloader::ModelIntegrity::NotInstalled);

            json!({
                "id": model.to_string(),
                "displayName": model.display_name(),
                "description": model.description(),
                "sizeBytes": model.model_size_bytes(),
                "englishOnly": model.is_english_only(),
                "active": *model == state.config.model,
                "integrity": integrity,
            })
        })
        .collect();

    Json(json!({ "models": models }))
}

/// `POST /api/models/{id}/download`, `GET .../progress`, `POST .../cancel`,
/// `DELETE /api/models/{id}`, `POST .../activate` — all Phase 2. Stubbed now
/// (rather than left un-routed) to freeze the `/api/*` contract shape per
/// `docs/stt-server-design.md` §11 Phase 1 goal: parallel Phase 2/3/4 agents
/// can code against these paths today.
///
/// Error envelope matches `hypr_transcribe_core::json_error_response`
/// (already used by `/v1/listen`): `{"error": "<code>", "detail": "<msg>"}`.
pub async fn not_implemented(Path(id): Path<String>) -> Response {
    json_error_response(
        StatusCode::NOT_IMPLEMENTED,
        "not_implemented",
        format!(
            "model management for `{id}` is not implemented in Phase 1 \
             (see docs/stt-server-design.md, Phase 2: model management API)"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::build_router;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn models_lists_the_full_whisper_catalog_as_not_installed() {
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
                    .uri("/api/models")
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
        let models = json["models"].as_array().unwrap();
        assert_eq!(models.len(), CATALOG.len());
        assert!(
            models
                .iter()
                .all(|m| m["integrity"]["state"] == "notInstalled")
        );
    }

    #[tokio::test]
    async fn model_mutations_return_501_with_the_shared_error_envelope() {
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
                    .method("POST")
                    .uri("/api/models/QuantizedSmall/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "not_implemented");
        assert!(json["detail"].as_str().unwrap().contains("QuantizedSmall"));
    }
}
