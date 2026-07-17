use std::sync::Arc;

use axum::{Json, extract::State, response::IntoResponse};
use hypr_local_model::LocalModel;
use hypr_model_downloader::ModelIntegrity;
use hypr_transcribe_core::SttEngine;
use serde_json::json;

use crate::state::AppState;

/// `GET /api/status` — always answers 200, even with no model installed
/// (see `docs/stt-server-design.md` §6). `loadedModel` reflects whichever
/// model is currently active (`AppState::active`, updated by
/// `POST /api/models/{id}/activate` in Phase 2) — `null` until that model
/// exists verified/present on disk. GPU `backends`/offload verification
/// land in Phase 4 (`list_ggml_backends` is release-build-only, see
/// `crates/whisper-local/src/ggml.rs`; it returns an empty list in this
/// debug build and on this CPU image either way).
pub async fn status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let active_model = state.active.read().await.model.clone();
    let model = LocalModel::Whisper(active_model.clone());
    let integrity =
        hypr_model_downloader::verify_model(&model, &state.config.model_dir).unwrap_or_else(
            |error| ModelIntegrity::Corrupt(format!("integrity check failed: {error}")),
        );

    let loaded_model = match &integrity {
        ModelIntegrity::Verified | ModelIntegrity::PresentUnverified => Some(json!({
            "id": active_model.to_string(),
            "file": active_model.file_name(),
            "path": state.model_path_for(&active_model).display().to_string(),
            "integrity": integrity,
        })),
        ModelIntegrity::NotInstalled | ModelIntegrity::Corrupt(_) => None,
    };

    let probe_guard = state.probe_result.read().await;
    let probe_realtime_factor = *probe_guard;
    drop(probe_guard);

    let backends = hypr_whisper_local::list_ggml_backends();
    let has_gpu = backends.iter().any(|b| b.kind == "GPU" || b.kind == "ACCEL");

    let gpu_offload = if !has_gpu {
        "cpu"
    } else {
        match probe_realtime_factor {
            Some(factor) if factor >= 1.5 => "verified",
            Some(_) => "cpu",
            None => "unknown",
        }
    };

    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": <hypr_transcribe_whisper_local::LoadedWhisper as SttEngine>::arch(),
        "loadedModel": loaded_model,
        "modelIntegrity": integrity,
        "backends": backends,
        "requireGpu": state.config.require_gpu,
        "gpuOffload": gpu_offload,
        "probeRealtimeFactor": probe_realtime_factor,
        "uptimeSecs": state.start_time.elapsed().as_secs(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use hypr_local_model::WhisperModel;
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
        assert_eq!(json["gpuOffload"], "cpu");
        assert!(json["probeRealtimeFactor"].is_null());
    }

    /// `/api/status.loadedModel` must track `AppState::active`, not the
    /// startup-configured default — the whole point of Phase 2's `activate`
    /// endpoint. `activate` itself rejects anything short of
    /// `ModelIntegrity::Verified`/`PresentUnverified` (see
    /// `admin::models::tests` for why those states can't be faked
    /// deterministically for a `WhisperModel` without real, correctly-hashed
    /// model bytes), so this only proves the *rejection* path leaves
    /// `loadedModel` unchanged; the success path is exercised end-to-end by
    /// the live smoke test (real download → activate → `/api/status`).
    #[tokio::test]
    async fn status_loaded_model_is_unchanged_when_activation_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::Config {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let state = Arc::new(AppState::new(config));

        let rejected = state.activate(WhisperModel::QuantizedBaseEn).await;
        assert!(rejected.is_err());

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
        // Still the startup default — the rejected activation never swapped
        // `AppState::active`.
        assert!(json["loadedModel"].is_null());
        assert_eq!(json["modelIntegrity"]["state"], "notInstalled");
    }
}
