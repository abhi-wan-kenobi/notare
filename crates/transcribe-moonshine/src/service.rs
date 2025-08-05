use tracing::{error, info};

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use std::sync::{Arc, Mutex};

use crate::MoonshineOnnxModel;

pub struct TranscribeService {
    model: Arc<Mutex<MoonshineOnnxModel>>,
}

impl TranscribeService {
    pub async fn new(
        encoder_path: impl AsRef<std::path::Path>,
        decoder_path: impl AsRef<std::path::Path>,
        model_name: &str,
    ) -> Result<Self, crate::Error> {
        let model = MoonshineOnnxModel::from_paths(encoder_path, decoder_path, model_name)?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
        })
    }

    pub async fn handle_websocket(self, ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.on_upgrade(move |socket| self.handle_socket(socket))
    }

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
