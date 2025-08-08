use tracing::{error, info};

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::FromRequestParts,
    http::Request,
    response::IntoResponse,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tower::Service;

use crate::{MoonshineModelSize, MoonshineOnnxModel};

#[derive(Clone)]
pub struct TranscribeService {
    model: Arc<Mutex<MoonshineOnnxModel>>,
}

#[derive(Default)]
pub struct TranscribeServiceBuilder {
    model_size: Option<MoonshineModelSize>,
    tokenizer_path: Option<String>,
    encoder_path: Option<String>,
    decoder_path: Option<String>,
}

impl TranscribeServiceBuilder {
    pub fn model_size(mut self, model_size: MoonshineModelSize) -> Self {
        self.model_size = Some(model_size);
        self
    }

    pub fn tokenizer_path(mut self, tokenizer_path: String) -> Self {
        self.tokenizer_path = Some(tokenizer_path);
        self
    }

    pub fn encoder_path(mut self, encoder_path: String) -> Self {
        self.encoder_path = Some(encoder_path);
        self
    }

    pub fn decoder_path(mut self, decoder_path: String) -> Self {
        self.decoder_path = Some(decoder_path);
        self
    }

    pub fn build(self) -> Result<TranscribeService, crate::Error> {
        let model = MoonshineOnnxModel::new(
            self.encoder_path.unwrap(),
            self.decoder_path.unwrap(),
            self.tokenizer_path.unwrap(),
            self.model_size.unwrap_or(MoonshineModelSize::Tiny),
        )?;

        Ok(TranscribeService {
            model: Arc::new(Mutex::new(model)),
        })
    }
}

impl TranscribeService {
    pub fn builder() -> TranscribeServiceBuilder {
        TranscribeServiceBuilder::default()
    }

    pub async fn handle_websocket(self, ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.on_upgrade(move |socket| self.handle_socket(socket))
    }
}

impl<B> Service<Request<B>> for TranscribeService
where
    B: Send + 'static,
{
    type Response = axum::response::Response;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let service = self.clone();

        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            let ws_upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                Ok(ws) => ws,
                Err(e) => {
                    return Ok(e.into_response());
                }
            };

            Ok(service.handle_websocket(ws_upgrade).await.into_response())
        })
    }
}

impl TranscribeService {
    async fn handle_socket(self, mut socket: WebSocket) {
        info!("WebSocket connection established");

        while let Some(msg) = socket.recv().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = self.process_audio(&mut socket, data.to_vec()).await {
                        error!("Error processing audio: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {
                    // Ignore other message types
                }
            }
        }
    }

    async fn process_audio(
        &self,
        socket: &mut WebSocket,
        audio_data: Vec<u8>,
    ) -> Result<(), crate::Error> {
        // Convert audio bytes to f32 samples
        // Assuming 16-bit PCM audio at 16kHz
        let samples = bytes_to_f32_samples(&audio_data);

        // Create [1, num_samples] array
        let audio_array =
            hypr_onnx::ndarray::Array2::from_shape_vec((1, samples.len()), samples)
                .map_err(|e| crate::Error::Shape(format!("Failed to create audio array: {}", e)))?;

        // Run inference
        let tokens = {
            let mut model = self.model.lock().unwrap();
            model.generate(audio_array, None)?
        };

        // Convert tokens to text (simplified - you'd need a proper tokenizer)
        let text = format!("Tokens: {:?}", tokens);

        // Send result back
        socket
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| crate::Error::Other(format!("Failed to send response: {}", e)))?;

        Ok(())
    }
}

fn bytes_to_f32_samples(bytes: &[u8]) -> Vec<f32> {
    // Convert 16-bit PCM to f32 samples
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / 32768.0
        })
        .collect()
}
