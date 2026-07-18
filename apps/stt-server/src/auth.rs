//! Optional shared-secret gate for `/v1/listen` and the `/api/models/*`
//! mutation routes (SEC hardening, see `docs/stt-server-design.md` §10 /
//! `SECURITY-REVIEW.md`). This server ships LAN-only and unauthenticated
//! **by design** — see the README's security warning — but an operator who
//! wants an extra shared-secret gate on top of that (e.g. a shared
//! flat/office LAN, or a tailnet sitting next to other devices) can set
//! `NOTARE_STT_TOKEN` / `--token`. Unset (the default), this middleware is a
//! complete no-op.
//!
//! Deliberately **not** required on `GET /health`, `GET /api/status`,
//! `GET /api/models`, or `GET /api/models/{id}/progress` (see
//! `admin::router`) — those are read-only status pings the desktop's "Test
//! connection" probe and the embedded admin page depend on being reachable
//! *before* the operator has necessarily typed the token in yet, and
//! `/api/status` no longer leaks anything sensitive now that SEC-05 dropped
//! the absolute model path from its response.
//!
//! No desktop-side code change is needed to use this: the existing
//! "Custom" STT provider's optional `api_key` field
//! (`apps/desktop/src/settings/ai/stt`, wired in commit 203bff832) is
//! already sent as `Authorization: Bearer <api_key>` on every batch/live
//! call (`crates/owhisper-client/src/live.rs:106-107`) and on the
//! `/api/status` Test-connection probe
//! (`apps/desktop/src/settings/ai/stt/connection-test.ts:97-99`) — set the
//! same value as `NOTARE_STT_TOKEN` on the server and it just works.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use hypr_transcribe_core::json_error_response;

use crate::state::AppState;

/// Applied via `axum::middleware::from_fn` closures at each protected
/// router (`router::build_router`'s `/v1/listen`, `admin::router`'s
/// mutation sub-router) rather than exposed as a reusable `Layer`, so
/// callers don't have to fight axum's per-router `State` type — this just
/// takes the `Arc<AppState>` it needs directly.
pub async fn require_bearer_token(state: Arc<AppState>, request: Request<Body>, next: Next) -> Response {
    let Some(expected) = state.config.token.as_deref() else {
        return next.run(request).await;
    };

    if is_authorized(request.headers(), expected) {
        next.run(request).await
    } else {
        json_error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid `Authorization: Bearer <token>` (this server has NOTARE_STT_TOKEN set)"
                .to_string(),
        )
    }
}

fn is_authorized(headers: &HeaderMap, expected_token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|presented| constant_time_eq(presented.as_bytes(), expected_token.as_bytes()))
}

/// Avoids a byte-at-a-time timing side channel on the token comparison
/// (`==` on `&str`/`&[u8]` short-circuits at the first differing byte).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_bearer(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn accepts_the_matching_token() {
        assert!(is_authorized(&headers_with_bearer("Bearer secret"), "secret"));
    }

    #[test]
    fn rejects_a_wrong_token() {
        assert!(!is_authorized(&headers_with_bearer("Bearer wrong"), "secret"));
    }

    #[test]
    fn rejects_a_missing_header() {
        assert!(!is_authorized(&HeaderMap::new(), "secret"));
    }

    #[test]
    fn rejects_a_non_bearer_scheme() {
        assert!(!is_authorized(&headers_with_bearer("Basic secret"), "secret"));
    }

    #[test]
    fn rejects_an_empty_presented_token() {
        assert!(!is_authorized(&headers_with_bearer("Bearer "), "secret"));
        assert!(!is_authorized(&headers_with_bearer("Bearer"), ""));
    }
}
