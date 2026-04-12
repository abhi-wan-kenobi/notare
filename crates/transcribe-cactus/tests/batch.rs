mod common;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

fn audio_wav_bytes() -> Vec<u8> {
    std::fs::read(hypr_data::english_1::AUDIO_PATH).expect("failed to read audio file")
}

fn listen_request() -> axum::http::request::Builder {
    Request::builder()
        .method("POST")
        .uri("/v1/listen?channels=1&sample_rate=16000&language=en")
        .header("content-type", "audio/wav")
}

async fn response_json(response: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).expect("response is not JSON")
}

use transcribe_cactus::TranscribeService;

use common::{invalid_model_path, model_path};

#[ignore = "requires local cactus model files"]
#[test]
fn e2e_batch() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let app = TranscribeService::builder()
            .model_path(model_path())
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        let wav_bytes = audio_wav_bytes();

        let url = format!(
            "http://{}/v1/listen?channels=1&sample_rate=16000&language=en",
            addr
        );
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("content-type", "audio/wav")
            .body(wav_bytes)
            .send()
            .await
            .expect("request failed");

        assert_eq!(response.status(), 200);
        let v: serde_json::Value = response.json().await.expect("response is not JSON");

        let transcript = v
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let transcript_lower = transcript.trim().to_lowercase();
        assert!(
            !transcript_lower.is_empty(),
            "expected non-empty transcript"
        );
        assert!(
            transcript_lower.contains("maybe")
                || transcript_lower.contains("this")
                || transcript_lower.contains("talking"),
            "transcript looks like a hallucination (got: {:?})",
            transcript_lower
        );
        assert!(
            v["metadata"]["duration"].as_f64().unwrap_or_default() > 0.0,
            "expected positive duration in metadata"
        );
        assert_eq!(v["metadata"]["channels"], 1);

        let _ = shutdown_tx.send(());
    });
}

#[tokio::test]
async fn invalid_model_path_returns_http_500_json_error() {
    let app = TranscribeService::builder()
        .model_path(invalid_model_path())
        .build()
        .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

    let response = app
        .oneshot(
            listen_request()
                .body(Body::from(audio_wav_bytes()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"], "model_load_failed");
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("model file not found"),
        "unexpected detail: {body:?}"
    );
}

#[tokio::test]
async fn health_starts_loading_then_fails_for_invalid_model_path() {
    let app = TranscribeService::builder()
        .model_path(invalid_model_path())
        .build()
        .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let health: hypr_cactus_model::CactusServiceHealth = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        health.status,
        hypr_cactus_model::CactusServiceStatus::Loading
    );

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let health: hypr_cactus_model::CactusServiceHealth = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        health.status,
        hypr_cactus_model::CactusServiceStatus::Failed
    );
    assert!(
        health
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("model file not found")
    );
}

#[tokio::test]
async fn invalid_model_path_returns_sse_error_event() {
    let app = TranscribeService::builder()
        .model_path(invalid_model_path())
        .build()
        .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

    let response = app
        .oneshot(
            listen_request()
                .header("accept", "text/event-stream")
                .body(Body::from(audio_wav_bytes()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"], "model_load_failed");
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("model file not found"),
        "unexpected detail: {body:?}"
    );
}
