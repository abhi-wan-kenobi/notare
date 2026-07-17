mod model;

pub use model::{MMPROJ_FILE, VoxtralError, VoxtralModel, WEIGHT_FILE};

use std::path::Path;
use std::sync::{Arc, Mutex};

use hypr_transcribe_core::{EngineError, EngineSegment, SttEngine, SttEngineSession};

/// A loaded Voxtral (llama.cpp `libmtmd`) model.
///
/// Both the LLM decoder (`LlamaModel`) and the mtmd audio-encoder context
/// must never be used concurrently by two `llama_decode` calls at once (the
/// underlying KV cache/eval state isn't safe for that), so — same shape as
/// `parakeet_onnx::LoadedParakeet` — every session shares one model behind a
/// mutex and channels serialize through it.
pub struct LoadedVoxtral {
    inner: Arc<Mutex<VoxtralModel>>,
}

impl hypr_model_manager::ModelLoader for LoadedVoxtral {
    type Error = VoxtralError;

    /// `path` is the model *directory* holding [`WEIGHT_FILE`] and
    /// [`MMPROJ_FILE`].
    fn load(path: &Path) -> Result<Self, Self::Error> {
        let model = VoxtralModel::new(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
        })
    }
}

pub struct VoxtralSession {
    inner: Arc<Mutex<VoxtralModel>>,
}

impl SttEngineSession for VoxtralSession {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
        let text = {
            let mut model = self
                .inner
                .lock()
                .map_err(|_| EngineError::new(VoxtralError::Poisoned.to_string()))?;
            model
                .transcribe_samples(samples)
                .map_err(EngineError::from_error)?
        };

        if text.is_empty() {
            return Ok(vec![]);
        }

        let chunk_duration = samples.len() as f64 / hypr_transcribe_core::TARGET_SAMPLE_RATE as f64;

        // libmtmd's audio path has no per-token timestamp output today (it's
        // an LLM decode loop, not an alignment model): the whole chunk
        // becomes one segment spanning its duration, same fallback shape
        // `parakeet_onnx::group_tokens_into_words` uses when it has no
        // reliable timestamps either.
        Ok(vec![EngineSegment {
            text,
            start: 0.0,
            end: chunk_duration,
            confidence: 1.0,
            language: None,
        }])
    }
}

impl SttEngine for LoadedVoxtral {
    type Session = VoxtralSession;

    /// Voxtral's transcription mode has no language-forcing knob wired up
    /// here (Phase A: batch transcription only); the requested languages are
    /// accepted and ignored, same as Parakeet's session.
    fn session(
        &self,
        _languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError> {
        Ok(VoxtralSession {
            inner: Arc::clone(&self.inner),
        })
    }

    fn arch() -> &'static str {
        "voxtral-llama"
    }
}
