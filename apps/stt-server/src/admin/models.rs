use std::str::FromStr;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use hypr_local_model::{LocalModel, WhisperModel};
use hypr_model_downloader::{DownloadStatus, ModelIntegrity};
use hypr_transcribe_core::json_error_response;
use serde_json::json;

use crate::state::{ActivateError, AppState};

/// The whisper.cpp catalog this server can ever serve (Phase 1/2 are
/// whisper-only, see `docs/stt-server-design.md` §2). Mirrors the `ALL`
/// fixture in `crates/whisper-local-model/src/lib.rs` tests; there is no
/// `WhisperModel::all()` in that crate today.
pub(crate) const CATALOG: &[WhisperModel] = &[
    WhisperModel::QuantizedTiny,
    WhisperModel::QuantizedTinyEn,
    WhisperModel::QuantizedBase,
    WhisperModel::QuantizedBaseEn,
    WhisperModel::QuantizedSmall,
    WhisperModel::QuantizedSmallEn,
    WhisperModel::QuantizedLargeTurbo,
];

fn parse_model_id(id: &str) -> Option<WhisperModel> {
    WhisperModel::from_str(id).ok()
}

fn model_not_found(id: &str) -> Response {
    json_error_response(
        StatusCode::NOT_FOUND,
        "model_not_found",
        format!("unknown model id `{id}`; see GET /api/models for the catalog"),
    )
}

/// Poll-based download progress for one model, shared by `GET /api/models`
/// (embedded per-entry) and `GET /api/models/{id}/progress` (standalone). A
/// plain HTTP/JSON server has no Tauri-event-style push channel to mirror
/// the desktop's `DownloadProgressPayload` emit, so Phase 2 picks polling
/// over the WS/SSE progress stream sketched in `docs/stt-server-design.md`
/// §6's table — documented as a deviation in the Phase 2 addendum there.
///
/// `status` is one of `"idle" | "downloading" | "completed" | "failed" |
/// "corrupt"`; `percent` (0-100) accompanies `downloading`/`completed`,
/// `detail` accompanies `failed`/`corrupt`.
async fn progress_snapshot(
    state: &AppState,
    model: &WhisperModel,
    integrity: &ModelIntegrity,
) -> serde_json::Value {
    let local = LocalModel::Whisper(model.clone());
    let id = model.to_string();

    // Authoritative: is a download literally in flight right now (per the
    // downloader's own registry), regardless of what the progress map last
    // recorded.
    if state.downloader.is_downloading(&local).await {
        let percent = match state
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&id)
        {
            Some(DownloadStatus::Downloading(p)) => Some(*p),
            _ => None,
        };
        return json!({ "status": "downloading", "percent": percent });
    }

    // Otherwise fall back to the last recorded event for this id (covers the
    // moment right after a download finishes/fails, before the next
    // integrity check would otherwise reveal it).
    let last = state
        .progress
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .cloned();
    if let Some(status) = last {
        return match status {
            DownloadStatus::Completed => json!({ "status": "completed", "percent": 100 }),
            DownloadStatus::Failed(reason) => json!({ "status": "failed", "detail": reason }),
            DownloadStatus::Downloading(p) => json!({ "status": "downloading", "percent": p }),
        };
    }

    // No download has ever been observed this process lifetime (e.g. right
    // after boot): derive from on-disk integrity instead.
    match integrity {
        ModelIntegrity::Verified | ModelIntegrity::PresentUnverified => {
            json!({ "status": "completed", "percent": 100 })
        }
        ModelIntegrity::NotInstalled => json!({ "status": "idle" }),
        ModelIntegrity::Corrupt(reason) => json!({ "status": "corrupt", "detail": reason }),
    }
}

/// `GET /api/models` — the whisper.cpp catalog with live on-disk
/// `ModelIntegrity` (`hypr_model_downloader::verify_model`, re-checked on
/// every request — fine at catalog size 7) and a per-model `progress`
/// snapshot.
pub async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let active_model = state.active.read().await.model.clone();

    let mut models = Vec::with_capacity(CATALOG.len());
    for model in CATALOG {
        let local = LocalModel::Whisper(model.clone());
        let integrity = hypr_model_downloader::verify_model(&local, &state.config.model_dir)
            .unwrap_or(ModelIntegrity::NotInstalled);
        let progress = progress_snapshot(&state, model, &integrity).await;

        models.push(json!({
            "id": model.to_string(),
            "displayName": model.display_name(),
            "description": model.description(),
            "sizeBytes": model.model_size_bytes(),
            "englishOnly": model.is_english_only(),
            "active": *model == active_model,
            "integrity": integrity,
            "progress": progress,
        }));
    }

    Json(json!({ "models": models }))
}

/// `POST /api/models/{id}/download` — start an async download via
/// `hypr_model_downloader::ModelDownloadManager` (multi-part where the
/// catalog defines it — not the case for any whisper model today, but the
/// manager handles it transparently either way). Responds `404` for an
/// unknown id, `409` if a download is already in flight, `200` no-op if the
/// model is already installed, `202` once a new download has been started.
pub async fn download_model(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(model) = parse_model_id(&id) else {
        return model_not_found(&id);
    };
    let local = LocalModel::Whisper(model);

    if state.downloader.is_downloading(&local).await {
        return json_error_response(
            StatusCode::CONFLICT,
            "already_downloading",
            format!("model `{id}` is already downloading"),
        );
    }

    match state.downloader.is_downloaded(&local).await {
        Ok(true) => {
            return (
                StatusCode::OK,
                Json(json!({ "id": id, "status": "alreadyInstalled" })),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(error) => {
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "integrity_check_failed",
                error.to_string(),
            );
        }
    }

    if let Err(error) = state.downloader.download(&local).await {
        return json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "download_failed_to_start",
            error.to_string(),
        );
    }

    (
        StatusCode::ACCEPTED,
        Json(json!({ "id": id, "status": "downloading" })),
    )
        .into_response()
}

/// `GET /api/models/{id}/progress` — the same `progress_snapshot` embedded
/// per-entry in `GET /api/models`, addressable for a single model. `404` for
/// an unknown id.
pub async fn model_progress(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(model) = parse_model_id(&id) else {
        return model_not_found(&id);
    };
    let local = LocalModel::Whisper(model.clone());
    let integrity = hypr_model_downloader::verify_model(&local, &state.config.model_dir)
        .unwrap_or(ModelIntegrity::NotInstalled);
    let progress = progress_snapshot(&state, &model, &integrity).await;

    Json(json!({ "id": id, "progress": progress })).into_response()
}

/// `POST /api/models/{id}/cancel` — cancel an in-flight download
/// (`ModelDownloadManager::cancel_download`). `404` unknown id, `409` if
/// nothing is downloading for it.
pub async fn cancel_download(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(model) = parse_model_id(&id) else {
        return model_not_found(&id);
    };
    let local = LocalModel::Whisper(model);

    match state.downloader.cancel_download(&local).await {
        Ok(true) => Json(json!({ "id": id, "status": "cancelled" })).into_response(),
        Ok(false) => json_error_response(
            StatusCode::CONFLICT,
            "not_downloading",
            format!("model `{id}` has no in-flight download to cancel"),
        ),
        Err(error) => json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cancel_failed",
            error.to_string(),
        ),
    }
}

/// `DELETE /api/models/{id}` — remove model files + integrity sidecars
/// (`ModelDownloadManager::delete`, which also clears the `.verified`
/// stamp). `404` unknown id, `409` if it is the currently active/loaded
/// model, `200` no-op if it was not installed, `200` on successful delete.
pub async fn delete_model(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(model) = parse_model_id(&id) else {
        return model_not_found(&id);
    };

    let active_model = state.active.read().await.model.clone();
    if model == active_model {
        return json_error_response(
            StatusCode::CONFLICT,
            "model_in_use",
            format!(
                "model `{id}` is the currently active/loaded model; \
                 activate a different model before deleting it"
            ),
        );
    }

    let local = LocalModel::Whisper(model);

    match state.downloader.is_downloaded(&local).await {
        Ok(false) => {
            return (
                StatusCode::OK,
                Json(json!({ "id": id, "status": "notInstalled" })),
            )
                .into_response();
        }
        Err(error) => {
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "integrity_check_failed",
                error.to_string(),
            );
        }
        Ok(true) => {}
    }

    if let Err(error) = state.downloader.delete(&local).await {
        return json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "delete_failed",
            error.to_string(),
        );
    }

    state
        .progress
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&id);

    (
        StatusCode::OK,
        Json(json!({ "id": id, "status": "deleted" })),
    )
        .into_response()
}

/// `POST /api/models/{id}/activate` — make this the model
/// `/v1/listen` serves and `/api/status.loadedModel` reports
/// (`AppState::activate`). `404` unknown id, `409` if the model is not
/// installed or fails integrity verification, `200` on success.
pub async fn activate_model(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(model) = parse_model_id(&id) else {
        return model_not_found(&id);
    };

    match state.activate(model).await {
        Ok(integrity) => Json(json!({
            "id": id,
            "status": "activated",
            "integrity": integrity,
        }))
        .into_response(),
        Err(ActivateError::NotInstalled) => json_error_response(
            StatusCode::CONFLICT,
            "model_not_installed",
            format!("model `{id}` is not installed; download it first"),
        ),
        Err(ActivateError::Corrupt(reason)) => json_error_response(
            StatusCode::CONFLICT,
            "model_corrupt",
            format!("model `{id}` failed integrity verification: {reason}"),
        ),
        Err(ActivateError::IntegrityCheckFailed(reason)) => json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "integrity_check_failed",
            reason,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::build_router;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn state_with_dir() -> (tempfile::TempDir, Arc<AppState>) {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::Config {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let state = Arc::new(AppState::new(config));
        (dir, state)
    }

    async fn send(app: axum::Router, method: &str, uri: &str) -> (StatusCode, serde_json::Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    /// Writes a sparse file of exactly the model's expected size at its
    /// install path, without writing (or downloading) any real content.
    /// `is_downloaded()` for a `WhisperModel` only checks existence + exact
    /// byte size (see `crates/local-model/src/lib.rs`), so this is enough to
    /// exercise "already installed" branches deterministically and without
    /// touching the network — it deliberately does *not* satisfy the CRC32
    /// check, so it is unsuitable for anything that calls `verify_model`
    /// expecting `Verified`.
    fn fake_installed(state: &AppState, model: &WhisperModel) {
        let path = state.model_path_for(model);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(model.model_size_bytes()).unwrap();
    }

    #[tokio::test]
    async fn models_lists_the_full_whisper_catalog_as_not_installed() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "GET", "/api/models").await;

        assert_eq!(status, StatusCode::OK);
        let models = json["models"].as_array().unwrap();
        assert_eq!(models.len(), CATALOG.len());
        assert!(
            models
                .iter()
                .all(|m| m["integrity"]["state"] == "notInstalled")
        );
        assert!(models.iter().all(|m| m["progress"]["status"] == "idle"));
        // The configured default model is active even before anything is
        // installed.
        assert!(
            models
                .iter()
                .any(|m| m["id"] == "QuantizedSmall" && m["active"] == true)
        );
    }

    #[tokio::test]
    async fn models_progress_reflects_a_corrupt_install() {
        let (_dir, state) = state_with_dir();
        // `fake_installed` writes a correct-*size* sparse file so
        // `is_downloaded()` (existence + size only) says "installed" and
        // `verify_model` proceeds to the CRC32 check — which a zero-filled
        // file will not coincidentally pass, landing deterministically on
        // `Corrupt` without downloading or hashing any real model bytes.
        let target = WhisperModel::QuantizedBaseEn;
        fake_installed(&state, &target);

        let app = build_router(state);
        let (status, json) = send(app, "GET", "/api/models").await;

        assert_eq!(status, StatusCode::OK);
        let models = json["models"].as_array().unwrap();
        let entry = models
            .iter()
            .find(|m| m["id"] == "QuantizedBaseEn")
            .unwrap();
        assert_eq!(entry["integrity"]["state"], "corrupt");
        assert_eq!(entry["progress"]["status"], "corrupt");
    }

    #[tokio::test]
    async fn download_unknown_model_is_404() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/NotAModel/download").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "model_not_found");
    }

    #[tokio::test]
    async fn download_already_installed_model_is_a_200_noop() {
        let (_dir, state) = state_with_dir();
        fake_installed(&state, &WhisperModel::QuantizedTiny);
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/QuantizedTiny/download").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "alreadyInstalled");
    }

    #[tokio::test]
    async fn progress_unknown_model_is_404() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "GET", "/api/models/NotAModel/progress").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "model_not_found");
    }

    #[tokio::test]
    async fn progress_not_installed_model_is_idle() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "GET", "/api/models/QuantizedTiny/progress").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["progress"]["status"], "idle");
    }

    #[tokio::test]
    async fn progress_is_idle_for_a_wrong_size_file() {
        let (_dir, state) = state_with_dir();
        let target = WhisperModel::QuantizedTiny;
        let path = state.model_path_for(&target);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // `is_downloaded()` for a whisper model checks existence *and* exact
        // size (`crates/local-model/src/lib.rs`) before `verify_model` ever
        // reaches its own checksum logic, so a wrong-size file reads as
        // plain `NotInstalled`/`idle`, not `Corrupt` — a same-size,
        // wrong-*content* file is what reaches `Corrupt` (see
        // `progress_reports_corrupt_for_a_checksum_mismatch`).
        std::fs::write(&path, b"not a real model").unwrap();

        let app = build_router(state);
        let (status, json) = send(app, "GET", "/api/models/QuantizedTiny/progress").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["progress"]["status"], "idle");
    }

    #[tokio::test]
    async fn progress_reports_corrupt_for_a_checksum_mismatch() {
        let (_dir, state) = state_with_dir();
        let target = WhisperModel::QuantizedTiny;
        // Correct size, wrong content: passes the `is_downloaded()` gate but
        // fails the CRC32 check in `verify_model` — deterministic `Corrupt`
        // without downloading or hashing any real model bytes.
        fake_installed(&state, &target);

        let app = build_router(state);
        let (status, json) = send(app, "GET", "/api/models/QuantizedTiny/progress").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["progress"]["status"], "corrupt");
        assert!(
            json["progress"]["detail"]
                .as_str()
                .unwrap()
                .contains("checksum")
        );
    }

    #[tokio::test]
    async fn cancel_unknown_model_is_404() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/NotAModel/cancel").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "model_not_found");
    }

    #[tokio::test]
    async fn cancel_with_nothing_in_flight_is_409() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/QuantizedTiny/cancel").await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "not_downloading");
    }

    #[tokio::test]
    async fn delete_unknown_model_is_404() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "DELETE", "/api/models/NotAModel").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "model_not_found");
    }

    #[tokio::test]
    async fn delete_not_installed_model_is_a_200_noop() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        // QuantizedTiny is not the configured default (QuantizedSmall), so
        // it is also not the active model — isolates this from the
        // model-in-use branch.
        let (status, json) = send(app, "DELETE", "/api/models/QuantizedTiny").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "notInstalled");
    }

    #[tokio::test]
    async fn delete_refuses_the_active_model() {
        let (_dir, state) = state_with_dir();
        fake_installed(&state, &WhisperModel::QuantizedSmall);
        let app = build_router(state);

        // QuantizedSmall is `Config::default().model` — the active model
        // from boot, with nothing else activated.
        let (status, json) = send(app, "DELETE", "/api/models/QuantizedSmall").await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "model_in_use");
    }

    #[tokio::test]
    async fn delete_removes_a_non_active_installed_model() {
        let (_dir, state) = state_with_dir();
        fake_installed(&state, &WhisperModel::QuantizedTiny);
        let path = state.model_path_for(&WhisperModel::QuantizedTiny);
        assert!(path.is_file());
        let app = build_router(state);

        let (status, json) = send(app, "DELETE", "/api/models/QuantizedTiny").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "deleted");
        assert!(!path.is_file());
    }

    #[tokio::test]
    async fn activate_unknown_model_is_404() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/NotAModel/activate").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "model_not_found");
    }

    #[tokio::test]
    async fn activate_not_installed_model_is_409() {
        let (_dir, state) = state_with_dir();
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/QuantizedTiny/activate").await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "model_not_installed");
    }

    #[tokio::test]
    async fn activate_corrupt_model_is_409() {
        let (_dir, state) = state_with_dir();
        // Correct size, wrong content — passes `is_downloaded()` but fails
        // the CRC32 check, landing on `Corrupt` (see `fake_installed`).
        fake_installed(&state, &WhisperModel::QuantizedTiny);
        let app = build_router(state);

        let (status, json) = send(app, "POST", "/api/models/QuantizedTiny/activate").await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "model_corrupt");
    }

    // A successful `activate` (`ModelIntegrity::Verified`) is deliberately
    // not covered here: `WhisperModel`'s catalog always carries a checksum
    // and `is_downloaded()` requires an exact-size *file* (see
    // `fake_installed`'s doc comment), so `Verified` can only be reached
    // with real, correctly-hashed model bytes, and
    // `ModelIntegrity::PresentUnverified` is unreachable for any
    // `WhisperModel` at all — `is_downloaded()` gates on `path.is_file()`
    // before `verify_model` ever reaches the directory branch that would
    // produce it. The success path (`activate` → 200 → `/api/status`
    // reflects it) is exercised end-to-end in the live smoke test instead
    // (see `docs/stt-server-design.md`, Phase 2 addendum, and the README).
}
