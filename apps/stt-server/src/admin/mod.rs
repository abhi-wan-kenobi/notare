//! `/api/*` — the management surface layered on top of the reused
//! `hypr_transcribe_core` router. Frozen route table in Phase 1 (all routes
//! existed, model-mutation ones answered `501`); implemented for real in
//! Phase 2 per `docs/stt-server-design.md` §11 (GPU-image/backend reporting
//! in `/api/status` is still Phase 4).

pub(crate) mod models;
mod status;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/status", get(status::status_handler))
        .route("/api/models", get(models::list_models))
        .route("/api/models/{id}/download", post(models::download_model))
        .route("/api/models/{id}/progress", get(models::model_progress))
        .route("/api/models/{id}/cancel", post(models::cancel_download))
        .route("/api/models/{id}", delete(models::delete_model))
        .route("/api/models/{id}/activate", post(models::activate_model))
        .with_state(state)
}
