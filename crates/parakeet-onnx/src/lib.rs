// Portions derived from Meetily (https://github.com/Zackriya-Solutions/meeting-minutes),
// Copyright (c) Zackriya Solutions, MIT License.

mod model;

pub use model::{ParakeetError, ParakeetModel, TimestampedResult};

use std::path::Path;
use std::sync::{Arc, Mutex};

use hypr_transcribe_core::{EngineError, EngineSegment, SttEngine, SttEngineSession};

/// Seconds of audio covered by one encoder frame
/// (`WINDOW_SIZE * SUBSAMPLING_FACTOR` = 10ms * 8 = 80ms).
const FRAME_SECONDS: f64 = (model::WINDOW_SIZE as f64) * (model::SUBSAMPLING_FACTOR as f64);

/// A loaded Parakeet TDT ONNX model.
///
/// The ~650MB int8 encoder must never be loaded twice, but `ort`'s
/// `Session::run` needs `&mut`, so the model sits behind a mutex that all
/// sessions share. Per-chunk decoding has no cross-chunk state, so
/// serializing chunk transcriptions across channels is correct (just not
/// parallel).
pub struct LoadedParakeet {
    inner: Arc<Mutex<ParakeetModel>>,
}

impl hypr_model_manager::ModelLoader for LoadedParakeet {
    type Error = ParakeetError;

    /// `path` is the model *directory* holding the fixed-name int8 ONNX
    /// files plus `vocab.txt`.
    fn load(path: &Path) -> Result<Self, Self::Error> {
        let model = ParakeetModel::new(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(model)),
        })
    }
}

pub struct ParakeetSession {
    inner: Arc<Mutex<ParakeetModel>>,
}

impl SttEngineSession for ParakeetSession {
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<EngineSegment>, EngineError> {
        let result = {
            let mut model = self
                .inner
                .lock()
                .map_err(|_| EngineError::new(ParakeetError::Poisoned.to_string()))?;
            model
                .transcribe_samples(samples.to_vec())
                .map_err(EngineError::from_error)?
        };

        let chunk_duration = samples.len() as f64 / hypr_transcribe_core::TARGET_SAMPLE_RATE as f64;
        Ok(group_tokens_into_words(
            &result.tokens,
            &result.timestamps,
            &result.text,
            chunk_duration,
        ))
    }
}

impl SttEngine for LoadedParakeet {
    type Session = ParakeetSession;

    /// Parakeet TDT v3 is inherently multilingual with no language forcing
    /// in this decoder; the requested languages are accepted and ignored.
    fn session(
        &self,
        _languages: Vec<hypr_language::Language>,
    ) -> Result<Self::Session, EngineError> {
        Ok(ParakeetSession {
            inner: Arc::clone(&self.inner),
        })
    }

    fn arch() -> &'static str {
        "parakeet-onnx"
    }
}

/// Group decoded tokens into per-word segments at space boundaries using the
/// per-token frame timestamps. Falls back to a single whole-chunk segment
/// when timestamps are missing or inconsistent.
fn group_tokens_into_words(
    tokens: &[String],
    timestamps: &[f32],
    full_text: &str,
    chunk_duration: f64,
) -> Vec<EngineSegment> {
    let text = full_text.trim();
    if text.is_empty() {
        return vec![];
    }

    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return vec![EngineSegment {
            text: text.to_string(),
            start: 0.0,
            end: chunk_duration.max(FRAME_SECONDS),
            confidence: 1.0,
            language: None,
        }];
    }

    let mut words: Vec<EngineSegment> = Vec::new();
    let mut current_text = String::new();
    let mut current_start = 0.0_f64;
    let mut current_end = 0.0_f64;

    for (token, &timestamp) in tokens.iter().zip(timestamps.iter()) {
        let starts_word = token.starts_with(' ');
        let piece = token.trim_start();
        let token_start = timestamp as f64;
        let token_end = token_start + FRAME_SECONDS;

        if starts_word && !current_text.is_empty() {
            words.push(EngineSegment {
                text: std::mem::take(&mut current_text),
                start: current_start,
                end: current_end,
                confidence: 1.0,
                language: None,
            });
        }

        if current_text.is_empty() {
            current_start = token_start;
        }
        current_text.push_str(piece);
        current_end = token_end.max(current_end);
    }

    if !current_text.is_empty() {
        words.push(EngineSegment {
            text: current_text,
            start: current_start,
            end: current_end,
            confidence: 1.0,
            language: None,
        });
    }

    // Purely-punctuation tokens can produce empty strings; drop them.
    words.retain(|word| !word.text.trim().is_empty());

    if words.is_empty() {
        return vec![EngineSegment {
            text: text.to_string(),
            start: 0.0,
            end: chunk_duration.max(FRAME_SECONDS),
            confidence: 1.0,
            language: None,
        }];
    }

    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn groups_tokens_into_words_at_space_boundaries() {
        // " he" "llo" " wor" "ld" -> "hello" + "world"
        let tokens = strings(&[" he", "llo", " wor", "ld"]);
        let timestamps = vec![0.0_f32, 0.08, 0.4, 0.48];

        let words = group_tokens_into_words(&tokens, &timestamps, "hello world", 1.0);

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "hello");
        assert!((words[0].start - 0.0).abs() < 1e-9);
        assert!((words[0].end - 0.16).abs() < 1e-6);
        assert_eq!(words[1].text, "world");
        assert!((words[1].start - 0.4).abs() < 1e-6);
        assert!((words[1].end - 0.56).abs() < 1e-6);
        assert!(words.iter().all(|w| w.confidence == 1.0));
        assert!(words.iter().all(|w| w.language.is_none()));
    }

    #[test]
    fn word_times_are_monotonic() {
        let tokens = strings(&[" a", " b", " c"]);
        let timestamps = vec![0.0_f32, 0.5, 1.2];

        let words = group_tokens_into_words(&tokens, &timestamps, "a b c", 2.0);

        assert_eq!(words.len(), 3);
        for pair in words.windows(2) {
            assert!(pair[0].start <= pair[1].start);
            assert!(pair[0].end <= pair[1].end + 1e-9);
        }
        for word in &words {
            assert!(word.end > word.start);
        }
    }

    #[test]
    fn falls_back_to_single_segment_without_timestamps() {
        let tokens = strings(&[" he", "llo"]);
        let timestamps: Vec<f32> = vec![]; // inconsistent with tokens

        let words = group_tokens_into_words(&tokens, &timestamps, "hello", 3.0);

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[0].end, 3.0);
    }

    #[test]
    fn empty_text_produces_no_segments() {
        let words = group_tokens_into_words(&[], &[], "  ", 3.0);
        assert!(words.is_empty());
    }

    #[test]
    fn leading_punctuation_token_attaches_to_word() {
        // punctuation token without a leading space joins the current word
        let tokens = strings(&[" hi", ","]);
        let timestamps = vec![0.0_f32, 0.08];

        let words = group_tokens_into_words(&tokens, &timestamps, "hi,", 1.0);

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "hi,");
    }
}
