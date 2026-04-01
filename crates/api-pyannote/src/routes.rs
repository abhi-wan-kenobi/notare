use std::str::FromStr;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;

use crate::{
    config::PyannoteConfig,
    error::{PyannoteError, Result},
};

#[derive(Clone)]
struct AppState {
    client: hypr_pyannote_cloud::Client,
}

#[derive(Debug, Deserialize)]
struct ListJobsQuery {
    #[serde(default)]
    skip: Option<f64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    take: Option<f64>,
}

pub fn router(config: PyannoteConfig) -> Router {
    let state = AppState {
        client: config.client().expect("failed to build pyannote client"),
    };

    Router::new()
        .route("/v1/diarize", post(diarize))
        .route("/v1/identify", post(identify))
        .route("/v1/jobs", get(list_jobs))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/media/input", post(create_media_input))
        .route("/v1/media/output", post(create_media_output))
        .route("/v1/test", get(test_key))
        .route("/v1/voiceprint", post(voiceprint))
        .with_state(state)
}

async fn diarize(
    State(state): State<AppState>,
    Json(body): Json<hypr_pyannote_cloud::types::DiarizeRequest>,
) -> Result<Response> {
    forward(state.client.diarize(&body).await).await
}

async fn identify(
    State(state): State<AppState>,
    Json(body): Json<hypr_pyannote_cloud::types::IdentifyRequest>,
) -> Result<Response> {
    forward(state.client.identify(&body).await).await
}

async fn list_jobs(
    State(state): State<AppState>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response> {
    let status = query
        .status
        .as_deref()
        .map(hypr_pyannote_cloud::types::GetJobsByTeamStatus::from_str)
        .transpose()
        .map_err(|_| PyannoteError::bad_request("Invalid status"))?;

    forward(
        state
            .client
            .get_jobs_by_team(query.skip, status, query.take)
            .await,
    )
    .await
}

async fn get_job(State(state): State<AppState>, Path(job_id): Path<String>) -> Result<Response> {
    forward(state.client.get_job_by_id(&job_id).await).await
}

async fn create_media_input(
    State(state): State<AppState>,
    Json(body): Json<hypr_pyannote_cloud::types::GetMediaUploadUrl>,
) -> Result<Response> {
    forward(state.client.get_media_upload_url(&body).await).await
}

async fn create_media_output(
    State(state): State<AppState>,
    Json(body): Json<hypr_pyannote_cloud::types::GetMediaUploadUrl>,
) -> Result<Response> {
    forward(state.client.get_media_download_url(&body).await).await
}

async fn test_key(State(state): State<AppState>) -> Result<Response> {
    forward(state.client.test_key().await).await
}

async fn voiceprint(
    State(state): State<AppState>,
    Json(body): Json<hypr_pyannote_cloud::types::VoiceprintRequest>,
) -> Result<Response> {
    forward(state.client.voiceprint(&body).await).await
}

async fn forward<T: serde::Serialize>(
    result: std::result::Result<
        hypr_pyannote_cloud::ResponseValue<T>,
        hypr_pyannote_cloud::Error<()>,
    >,
) -> Result<Response> {
    let response = match result {
        Ok(response) => response,
        Err(error) => return Err(map_client_error(error).await),
    };

    let status = status_code(response.status());
    Ok((status, Json(response.into_inner())).into_response())
}

async fn map_client_error(error: hypr_pyannote_cloud::Error<()>) -> PyannoteError {
    match error {
        hypr_pyannote_cloud::Error::ErrorResponse(response) => {
            let status = status_code(response.status());
            PyannoteError::upstream(status, default_message(status))
        }
        hypr_pyannote_cloud::Error::UnexpectedResponse(response) => {
            let status = status_code(response.status());
            let body = response.text().await.unwrap_or_default();
            let message = error_message_from_body(&body).unwrap_or_else(|| default_message(status));
            PyannoteError::upstream(status, message)
        }
        hypr_pyannote_cloud::Error::CommunicationError(err)
        | hypr_pyannote_cloud::Error::InvalidUpgrade(err)
        | hypr_pyannote_cloud::Error::ResponseBodyError(err) => {
            PyannoteError::bad_gateway(err.to_string())
        }
        hypr_pyannote_cloud::Error::InvalidRequest(message)
        | hypr_pyannote_cloud::Error::Custom(message) => PyannoteError::bad_request(message),
        hypr_pyannote_cloud::Error::InvalidResponsePayload(_, err) => {
            PyannoteError::bad_gateway(err.to_string())
        }
    }
}

fn status_code(status: reqwest::StatusCode) -> StatusCode {
    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY)
}

fn error_message_from_body(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }

    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                })
        })
        .or_else(|| Some(body.to_string()))
}

fn default_message(status: StatusCode) -> String {
    match status {
        StatusCode::BAD_REQUEST => "Invalid request".to_string(),
        StatusCode::PAYMENT_REQUIRED => "Subscription is required".to_string(),
        StatusCode::TOO_MANY_REQUESTS => "Too many requests".to_string(),
        StatusCode::NOT_FOUND => "Resource not found".to_string(),
        _ => status
            .canonical_reason()
            .unwrap_or("Upstream request failed")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, body::to_bytes, http::Request, http::StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::config::PyannoteConfig;

    fn router(server: &MockServer) -> Router {
        super::router(PyannoteConfig {
            api_key: "pyannote-key".to_string(),
            api_base: server.uri(),
        })
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn diarize_forwards_body_and_auth_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/diarize"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jobId": "job-123",
                "status": "created"
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/diarize")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"https://example.com/audio.wav"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({"jobId": "job-123", "status": "created"})
        );

        let requests = server.received_requests().await.unwrap();
        let request = &requests[0];
        assert_eq!(request.method.as_str(), "POST");
        assert_eq!(request.url.path(), "/v1/diarize");
        assert_eq!(
            request
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer pyannote-key"
        );
        assert_eq!(
            request.body_json::<Value>().unwrap()["url"],
            json!("https://example.com/audio.wav")
        );
    }

    #[tokio::test]
    async fn list_jobs_forwards_query_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/jobs"))
            .and(header("authorization", "Bearer pyannote-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::get("/v1/jobs?skip=1&take=5&status=running")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let requests = server.received_requests().await.unwrap();
        let query = requests[0].url.query().unwrap();
        assert!(query.contains("skip=1"));
        assert!(query.contains("take=5"));
        assert!(query.contains("status=running"));
    }

    #[tokio::test]
    async fn media_input_preserves_created_status() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/media/input"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({"url": "https://upload.example.com"})),
            )
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(
                Request::post("/v1/media/input")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"url":"media://meeting"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn upstream_bad_request_maps_to_char_error_shape() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/test"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"message": "Invalid key"})),
            )
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(Request::get("/v1/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "Invalid request"}})
        );
    }

    #[tokio::test]
    async fn upstream_rate_limit_maps_to_char_error_shape() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/test"))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({"message": "Slow down"})))
            .mount(&server)
            .await;

        let response = router(&server)
            .oneshot(Request::get("/v1/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "rate_limited", "message": "Too many requests"}})
        );
    }
}
