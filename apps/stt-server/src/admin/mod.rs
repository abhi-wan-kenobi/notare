//! `/api/*` — the management surface layered on top of the reused
//! `hypr_transcribe_core` router. Frozen in Phase 1 (routes exist, some
//! return 501); implemented for real in Phase 2/4 per
//! `docs/stt-server-design.md` §11.

mod models;
mod status;
mod web;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(web::index))
        .route("/api/status", get(status::status_handler))
        .route("/api/models", get(models::list_models))
        .route("/api/models/{id}/download", post(models::not_implemented))
        .route("/api/models/{id}/progress", get(models::not_implemented))
        .route("/api/models/{id}/cancel", post(models::not_implemented))
        .route("/api/models/{id}", delete(models::not_implemented))
        .route("/api/models/{id}/activate", post(models::not_implemented))
        .with_state(state)
}
