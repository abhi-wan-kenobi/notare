//! Serve the local whisper transcription service for manual smoke tests.
//!
//! ```sh
//! cargo run -p transcribe-whisper-local --example serve -- <model.bin> [port]
//! curl -s -X POST "http://127.0.0.1:8787/v1/listen?channels=1&sample_rate=16000" \
//!   -H "content-type: audio/wav" --data-binary @audio.wav | jq
//! ```

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model_path = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap()
                .join("hyprnote/models/stt/ggml-small-q8_0.bin")
        });
    let port: u16 = std::env::args()
        .nth(2)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);

    let app = transcribe_whisper_local::TranscribeService::builder()
        .model_path(model_path.clone())
        .build()
        .into_router(
            |err: String| async move { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err) },
        );

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!(
        "serving {} on http://127.0.0.1:{port} (POST /v1/listen, WS /v1/listen)",
        model_path.display()
    );
    axum::serve(listener, app).await?;
    Ok(())
}
