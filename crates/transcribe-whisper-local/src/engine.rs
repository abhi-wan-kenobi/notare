use std::path::Path;

use hypr_transcribe_core::{EngineError, EngineSegment, SttEngine, SttEngineSession};

/// Loaded whisper.cpp model, wrapped so this crate can implement the
/// engine traits from `hypr-transcribe-core` (orphan rule).
pub struct LoadedWhisper {
    inner: hypr_whisper_local::LoadedWhisper,
}

impl LoadedWhisper {
    pub fn inner(&self) -> &hypr_whisper_local::LoadedWhisper {
        &self.inner
    }
}

impl hypr_model_manager::ModelLoader for LoadedWhisper {
    type Error = hypr_whisper_local::Error;

    fn load(path: &Path) -> Result<Self, Self::Error> {
        let inner = hypr_whisper_local::LoadedWhisper::builder()
            .model_path(path.to_string_lossy().into_owned())
            .build()?;
        Ok(Self { inner })
    }
}

/// Per-channel whisper.cpp session (wraps a whisper state on the shared
/// context owned by the loaded model).
pub struct WhisperSession {
    inner: hypr_whisper_local::Whisper,
}

impl SttEngineSession for WhisperSession {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
        let segments = self
            .inner
            .transcribe(samples)
            .map_err(EngineError::from_error)?;

        Ok(segments
            .into_iter()
            .map(|segment| EngineSegment {
                text: segment.text().to_string(),
                start: segment.start(),
                end: segment.end(),
                confidence: segment.confidence() as f64,
                language: segment.language().map(|value| value.to_string()),
            })
            .collect())
    }
}

impl SttEngine for LoadedWhisper {
    type Session = WhisperSession;

    fn session(
        &self,
        languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError> {
        let whisper_languages: Vec<hypr_whisper::Language> = languages
            .into_iter()
            .filter_map(|language| language.try_into().ok())
            .collect();

        let inner = self
            .inner
            .session(whisper_languages)
            .map_err(EngineError::from_error)?;
        Ok(WhisperSession { inner })
    }

    fn arch() -> &'static str {
        "whisper-local"
    }
}
