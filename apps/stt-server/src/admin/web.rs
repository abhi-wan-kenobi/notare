use axum::response::{Html, IntoResponse};

/// `GET /` — the embedded web admin page (Phase 3, `docs/stt-server-design.md`
/// §9). Static, self-contained, no state needed — the page fetches
/// `/api/status` and `/api/models` itself once loaded in the browser.
pub async fn index() -> impl IntoResponse {
    Html(crate::assets::INDEX_HTML)
}

#[cfg(test)]
mod tests {
    use crate::router::build_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn index_serves_the_admin_page_as_html() {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::Config {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let state = Arc::new(crate::state::AppState::new(config));
        let app = build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(
            content_type.starts_with("text/html"),
            "expected text/html, got `{content_type}`"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("<title>"));
        assert!(text.contains("Notare"));
        // The fetch layer must target the frozen /api/* contract.
        assert!(text.contains("/api/status"));
        assert!(text.contains("/api/models"));
    }
}
