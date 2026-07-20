mod model;

pub use model::{MMPROJ_FILE, TranscribeTarget, VoxtralError, VoxtralModel, WEIGHT_FILE};

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
    /// The languages requested for this session. The first non-English entry
    /// is the transcription target; an empty list or an English-first request
    /// selects the default English verbatim path.
    languages: Vec<hypr_language::Language>,
}

impl SttEngineSession for VoxtralSession {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
        let (target, language_code) = resolve_target(&self.languages);

        let text = {
            let mut model = self
                .inner
                .lock()
                .map_err(|_| EngineError::new(VoxtralError::Poisoned.to_string()))?;
            model
                .transcribe_samples(samples, &target)
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
            language: Some(language_code),
        }])
    }
}

impl SttEngine for LoadedVoxtral {
    type Session = VoxtralSession;

    /// The first requested language becomes the transcription target; an
    /// empty list or an English-first request selects the default English
    /// verbatim path. (Phase A: batch-per-utterance only — no token
    /// streaming, no cross-chunk state, per issue #36.)
    fn session(
        &self,
        languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError> {
        Ok(VoxtralSession {
            inner: Arc::clone(&self.inner),
            languages,
        })
    }

    fn arch() -> &'static str {
        "voxtral-llama"
    }
}

/// Resolve the requested language set into a Voxtral transcription target and
/// the ISO code to stamp on the output segment.
///
/// - Both Hindi and English present → **Hinglish** (romanized code-mix), the
///   common Indian-office case (labelled `hi-Latn` = romanized Hindi).
/// - A single non-English language first → that language in its native script.
/// - Otherwise (empty, or English-first) → English.
///
/// The full script-preference toggle (Hinglish-in-Devanagari, Hindi-in-Roman)
/// waits on the request-param plumbing tracked in issue #40.
fn resolve_target(languages: &[hypr_language::Language]) -> (TranscribeTarget, String) {
    let has = |code: &str| languages.iter().any(|l| l.iso639_code() == code);
    if has("hi") && has("en") {
        return (TranscribeTarget::HinglishRoman, "hi-Latn".to_string());
    }
    match languages.first() {
        Some(language) if language.iso639_code() != "en" => (
            TranscribeTarget::Language(language.clone()),
            language.iso639_code().to_string(),
        ),
        _ => (TranscribeTarget::English, "en".to_string()),
    }
}

#[cfg(test)]
mod resolve_target_tests {
    use super::*;

    fn lang(code: &str) -> hypr_language::Language {
        code.parse().unwrap()
    }

    #[test]
    fn hindi_plus_english_is_hinglish_roman() {
        let (target, code) = resolve_target(&[lang("hi"), lang("en")]);
        assert_eq!(target, TranscribeTarget::HinglishRoman);
        assert_eq!(code, "hi-Latn");
        // order-independent: English first, Hindi second still reads as Hinglish.
        assert_eq!(
            resolve_target(&[lang("en"), lang("hi")]).0,
            TranscribeTarget::HinglishRoman
        );
    }

    #[test]
    fn hindi_alone_is_devanagari_language() {
        let (target, code) = resolve_target(&[lang("hi")]);
        assert_eq!(target, TranscribeTarget::Language(lang("hi")));
        assert_eq!(code, "hi");
    }

    #[test]
    fn empty_or_english_is_english() {
        assert_eq!(resolve_target(&[]).0, TranscribeTarget::English);
        assert_eq!(resolve_target(&[lang("en")]).0, TranscribeTarget::English);
    }
}
