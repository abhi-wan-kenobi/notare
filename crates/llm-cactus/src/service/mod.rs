use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use tower::Service;

use crate::ModelManager;

use self::{
    request::{ChatCompletionRequest, apply_response_format, build_options, convert_messages},
    response::{
        build_non_streaming_response, build_streaming_response, status_code_for_model_error,
    },
};

mod request;
mod response;

type CactusModelManager = ModelManager<hypr_cactus::Model>;

pub const COMPLETE_PATH: &str = "/v1/chat/completions";
pub const HEALTH_PATH: &str = "/health";

#[derive(Clone)]
pub struct CompleteService {
    manager: CactusModelManager,
    health: hypr_cactus::ServiceHealthTracker,
    model_label: Option<String>,
}

impl CompleteService {
    pub fn new(manager: CactusModelManager) -> Self {
        hypr_cactus::init_runtime();

        let health = hypr_cactus::ServiceHealthTracker::new();
        let model_label = manager.default_model_name();
        if model_label.is_some() {
            let warmup_manager = manager.clone();
            let warmup_health = health.clone();
            tokio::spawn(async move {
                tokio::task::yield_now().await;
                match warmup_manager.get(None).await {
                    Ok(_) => warmup_health.mark_ready(),
                    Err(error) => warmup_health.mark_load_failed(error.to_string()),
                }
            });
        } else {
            health.mark_idle();
        }

        Self {
            manager,
            health,
            model_label,
        }
    }

    pub fn into_router<F, Fut>(self, on_error: F) -> axum::Router
    where
        F: FnOnce(crate::Error) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = (StatusCode, String)> + Send,
    {
        let health = self.health.clone();
        let service = axum::error_handling::HandleError::new(self, on_error);

        axum::Router::new()
            .route(
                HEALTH_PATH,
                axum::routing::get(move || {
                    let health = health.clone();
                    async move { axum::Json(health.snapshot()) }
                }),
            )
            .route_service(COMPLETE_PATH, service)
    }
}

impl Service<Request<Body>> for CompleteService {
    type Response = Response;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let manager = self.manager.clone();
        let health = self.health.clone();
        let model_label = self.model_label.clone();

        Box::pin(async move {
            let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
                Ok(b) => b,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let request: ChatCompletionRequest = match serde_json::from_slice(&body_bytes) {
                Ok(r) => r,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let mut messages = match convert_messages(&request.messages) {
                Ok(messages) => messages,
                Err(error) => {
                    return Ok((StatusCode::BAD_REQUEST, error).into_response());
                }
            };
            if let Err(error) = hypr_cactus::validate_messages(&messages) {
                return Ok((status_code_for_model_error(&error), error.to_string()).into_response());
            }
            let mut options = build_options(&request);
            apply_response_format(
                request.response_format.as_ref(),
                &mut messages,
                &mut options,
            );

            let model = match manager.get(None).await {
                Ok(m) => {
                    health.mark_ready();
                    m
                }
                Err(e) => {
                    health.mark_load_failed(e.to_string());
                    return Ok((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response());
                }
            };

            if request.stream.unwrap_or(false) {
                let completion_stream =
                    match hypr_cactus::complete_stream(&model, messages, options) {
                        Ok(s) => s,
                        Err(e) => {
                            return Ok(
                                (status_code_for_model_error(&e), e.to_string()).into_response()
                            );
                        }
                    };

                Ok(build_streaming_response(completion_stream, &model_label))
            } else {
                Ok(build_non_streaming_response(&model, messages, options, &model_label).await)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
    use tower::ServiceExt;

    use super::*;

    async fn read_health(
        app: axum::Router,
    ) -> (StatusCode, hypr_cactus_model::CactusServiceHealth) {
        let response = app
            .oneshot(
                Request::builder()
                    .uri(HEALTH_PATH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload = serde_json::from_slice(&body).unwrap();
        (status, payload)
    }

    #[tokio::test]
    async fn health_starts_loading_then_fails_when_default_model_cannot_load() {
        let model_path =
            std::env::temp_dir().join(format!("llm-cactus-missing-model-{}", uuid::Uuid::new_v4()));
        let manager = ModelManager::<hypr_cactus::Model>::builder()
            .register("default", &model_path)
            .default_model("default")
            .build();
        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let (status, loading) = read_health(app.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            loading.status,
            hypr_cactus_model::CactusServiceStatus::Loading
        );

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;

        let (_, failed) = read_health(app).await;
        assert_eq!(
            failed.status,
            hypr_cactus_model::CactusServiceStatus::Failed
        );
        assert!(
            failed
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("model file not found")
        );
    }

    #[tokio::test]
    async fn health_starts_idle_without_default_model() {
        let manager = ModelManager::<hypr_cactus::Model>::builder().build();
        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let (status, idle) = read_health(app).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(idle.status, hypr_cactus_model::CactusServiceStatus::Idle);
    }

    #[tokio::test]
    async fn request_with_model_field_reports_default_model_failure() {
        let manager = ModelManager::<hypr_cactus::Model>::builder().build();
        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(COMPLETE_PATH)
                    .body(Body::from(
                        serde_json::json!({
                            "model": "missing",
                            "messages": [{"role": "user", "content": "hello"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("no default model configured"));

        let (_, health) = read_health(app).await;
        assert_eq!(
            health.status,
            hypr_cactus_model::CactusServiceStatus::Failed
        );
        assert_eq!(health.error.as_deref(), Some("no default model configured"));
    }

    #[tokio::test]
    async fn request_model_field_does_not_select_a_named_model() {
        let model_path =
            std::env::temp_dir().join(format!("llm-cactus-missing-model-{}", uuid::Uuid::new_v4()));
        let manager = ModelManager::<hypr_cactus::Model>::builder()
            .register("named-model", &model_path)
            .build();
        let app = CompleteService::new(manager)
            .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()) });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(COMPLETE_PATH)
                    .body(Body::from(
                        serde_json::json!({
                            "model": "named-model",
                            "messages": [{"role": "user", "content": "hello"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("no default model configured"));
    }
}
