use std::path::Path;

/// Error produced by an STT engine implementation.
///
/// Engines carry their own rich error types; at the service boundary we only
/// need something displayable, so implementations map into this.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct EngineError {
    message: String,
}

impl EngineError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn from_error(error: impl std::error::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<String> for EngineError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for EngineError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// A transcription segment as produced by an engine for a single audio chunk.
///
/// `start`/`end` are chunk-relative seconds (the service maps them onto the
/// stream timeline).
#[derive(Debug, Clone, Default)]
pub struct EngineSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub confidence: f64,
    pub language: Option<String>,
}

/// A per-channel transcription session. Sessions are created per websocket
/// channel (or per batch request) and fed VAD-chunked audio.
///
/// `Unpin` because sessions are moved around as plain values inside the
/// service's polling streams.
pub trait SttEngineSession: Send + Unpin + 'static {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError>;
}

/// A loaded STT model that can mint transcription sessions.
///
/// The `ModelLoader` supertrait lets `hypr_model_manager::ModelManager`
/// own load/unload/keep-alive lifecycles for any engine.
pub trait SttEngine: hypr_model_manager::ModelLoader + Send + Sync + Sized + 'static {
    type Session: SttEngineSession;

    fn session(
        &self,
        languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError>;

    /// Short identifier for this runtime, e.g. `"whisper-local"` or
    /// `"parakeet-onnx"`. Used in stream metadata and error payloads.
    fn arch() -> &'static str;
}

/// Load a model through its `ModelLoader` impl, normalizing the error.
pub fn load_engine<E: SttEngine>(model_path: &Path) -> Result<E, EngineError> {
    E::load(model_path).map_err(|e| EngineError::new(e.to_string()))
}
