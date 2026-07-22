//! `/api/*` — the management surface layered on top of the reused
//! `hypr_transcribe_core` router. Frozen route table in Phase 1 (all routes
//! existed, model-mutation ones answered `501`); implemented for real in
//! Phase 2 per `docs/stt-server-design.md` §11 (GPU-image/backend reporting
//! in `/api/status` is still Phase 4).

pub(crate) mod models;
mod status;
mod web;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::middleware::from_fn;
use axum::routing::{delete, get, post};

use crate::auth::require_bearer_token;
use crate::state::AppState;

/// `GET /`, `GET /api/status`, `GET /api/models`, `GET
/// /api/models/{id}/progress` stay open even when `NOTARE_STT_TOKEN` is
/// configured (see `crate::auth`'s doc comment for why); the four
/// state-mutating routes below sit behind the token gate.
pub fn router(state: Arc<AppState>) -> Router {
    let auth_state = state.clone();
    let protected = Router::new()
        .route("/api/models/{id}/download", post(models::download_model))
        .route("/api/models/{id}/cancel", post(models::cancel_download))
        .route("/api/models/{id}", delete(models::delete_model))
        .route("/api/models/{id}/activate", post(models::activate_model))
        .layer(from_fn(move |req: Request<Body>, next| {
            let state = auth_state.clone();
            async move { require_bearer_token(state, req, next).await }
        }));

    Router::new()
        .route("/", get(web::index))
        .route("/dashboard", get(web::dashboard))
        .route("/api/sessions", get(web::sessions_handler))
        .route("/api/status", get(status::status_handler))
        .route("/api/models", get(models::list_models))
        .route("/api/models/{id}/progress", get(models::model_progress))
        .merge(protected)
        .with_state(state)
}
