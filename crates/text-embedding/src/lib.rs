//! On-device text embeddings for semantic search (Notare 0.5, WS-B1).
//!
//! Runs EmbeddingGemma-300M (ONNX, int8-quantized `onnx-community` export) on
//! the workspace's existing `ort` linkage via `hypr-onnx` — no new onnxruntime
//! is introduced (S0 spike decision, 2026-07-22: Option B, see README).
//!
//! # Prompt prefixes are part of the model contract
//!
//! EmbeddingGemma embeds queries and documents into the same space ONLY when
//! the model-card prompt prefixes are applied. They are applied INSIDE this
//! crate (`embed_query` / `embed_docs`) and deliberately not exported, so no
//! caller can accidentally embed un-prefixed text (the "prefix-trap" — a query
//! embedded as a document ranks incorrectly; the same text embedded both ways
//! only reaches ~0.86 cosine similarity).
//!
//! Exact strings, from the EmbeddingGemma model card ("Prompt Instructions"):
//! - query:    `task: search result | query: {content}`
//! - document: `title: none | text: {content}`
//!
//! # Output space
//!
//! The ONNX graph bakes in mask-weighted mean-pooling and L2-normalization
//! (`sentence_embedding` output, 768-d, unit norm). This crate then applies
//! Matryoshka truncation to [`EMBEDDING_DIM`] = 512 dims and re-normalizes,
//! matching the model card's documented 768→512 MRL recipe.

mod manifest;

pub use manifest::{ARTIFACTS, Artifact, MODEL_DIR_NAME, verify_artifacts};

use hypr_onnx::{
    ndarray::Array2,
    ort::{self, session::Session, value::TensorRef},
};
use tokenizers::Tokenizer;

/// Dimensionality of the embeddings this crate produces (Matryoshka-truncated
/// from the model's native 768). Must match the `vec0` column declared by the
/// `embedding_chunks` migration.
pub const EMBEDDING_DIM: usize = 512;

/// Native output dimensionality of EmbeddingGemma-300M.
pub const NATIVE_DIM: usize = 768;

/// Max input tokens (EmbeddingGemma context window is 2048; keep headroom for
/// the prompt prefix). Inputs longer than this are truncated by the tokenizer.
pub const MAX_TOKENS: usize = 2048;

const QUERY_PREFIX: &str = "task: search result | query: ";
const DOC_PREFIX: &str = "title: none | text: ";

const INPUT_IDS: &str = "input_ids";
const ATTENTION_MASK: &str = "attention_mask";
const OUTPUT_SENTENCE_EMBEDDING: &str = "sentence_embedding";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("model artifact missing or unreadable: {0}")]
    ModelIo(#[from] std::io::Error),
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
    #[error("onnxruntime error: {0}")]
    Ort(#[from] ort::Error),
    #[error("model output '{0}' missing from session outputs")]
    MissingOutput(&'static str),
    #[error("model returned a non-finite embedding")]
    NonFiniteEmbedding,
    #[error("artifact integrity check failed for {name}: expected sha256 {expected}, got {actual}")]
    IntegrityMismatch {
        name: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("empty input")]
    EmptyInput,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A loaded EmbeddingGemma session. Construction is expensive (~300 MB model
/// load); callers should hold one instance and drop it to release memory
/// (lazy-load / idle-unload policy lives in the consuming plugin, not here).
pub struct TextEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl TextEmbedder {
    /// Load from a directory containing the artifacts described by
    /// [`ARTIFACTS`] (`model_quantized.onnx`, `model_quantized.onnx_data`,
    /// `tokenizer.json`), e.g. the app-data model cache populated by the
    /// download manager.
    ///
    /// Uses `commit_from_file` (NOT `hypr_onnx::load_model_from_bytes`):
    /// the quantized export stores weights as ONNX external data, which
    /// onnxruntime resolves relative to the model *path* — loading from
    /// memory would break that resolution.
    pub fn load(model_dir: impl AsRef<std::path::Path>) -> Result<Self> {
        let model_dir = model_dir.as_ref();
        let model_path = model_dir.join(manifest::MODEL_FILE);
        let tokenizer_path = model_dir.join(manifest::TOKENIZER_FILE);

        let session = Session::builder()?
            .with_intra_threads(1)?
            .with_inter_threads(1)?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)?
            .commit_from_file(&model_path)?;

        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| Error::Tokenizer(e.to_string()))?;

        Ok(Self { session, tokenizer })
    }

    /// Embed a search query. The query prompt prefix is applied internally.
    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        if text.trim().is_empty() {
            return Err(Error::EmptyInput);
        }
        let mut out = self.embed_prefixed(&[format!("{QUERY_PREFIX}{text}")])?;
        Ok(out.pop().expect("one embedding for one input"))
    }

    /// Embed document/chunk texts for indexing. The document prompt prefix is
    /// applied internally. Returns one [`EMBEDDING_DIM`]-d unit vector per
    /// input, in order.
    pub fn embed_docs(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if texts.iter().any(|t| t.trim().is_empty()) {
            return Err(Error::EmptyInput);
        }
        let prefixed: Vec<String> = texts.iter().map(|t| format!("{DOC_PREFIX}{t}")).collect();
        self.embed_prefixed(&prefixed)
    }

    fn embed_prefixed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // tokenizer.json's post-processor adds <bos>/<eos>; padding is done
        // manually here (id 0, mask 0) to match the reference pipeline.
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;

        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(MAX_TOKENS))
            .max()
            .unwrap_or(0)
            .max(1);

        let batch = encodings.len();
        let mut ids = Array2::<i64>::zeros((batch, max_len));
        let mut mask = Array2::<i64>::zeros((batch, max_len));
        for (row, enc) in encodings.iter().enumerate() {
            let take = enc.get_ids().len().min(max_len);
            for col in 0..take {
                ids[[row, col]] = enc.get_ids()[col] as i64;
                mask[[row, col]] = enc.get_attention_mask()[col] as i64;
            }
        }

        let inputs = ort::inputs![
            INPUT_IDS => TensorRef::from_array_view(ids.view())?,
            ATTENTION_MASK => TensorRef::from_array_view(mask.view())?,
        ];
        let outputs = self.session.run(inputs)?;
        let embedding = outputs
            .get(OUTPUT_SENTENCE_EMBEDDING)
            .ok_or(Error::MissingOutput(OUTPUT_SENTENCE_EMBEDDING))?
            .try_extract_array::<f32>()?;

        let mut result = Vec::with_capacity(batch);
        for row in embedding.rows() {
            let full: Vec<f32> = row.iter().copied().collect();
            if full.iter().any(|v| !v.is_finite()) {
                return Err(Error::NonFiniteEmbedding);
            }
            result.push(truncate_and_renormalize(&full));
        }
        Ok(result)
    }
}

/// Matryoshka truncation: keep the first [`EMBEDDING_DIM`] components, then
/// L2-normalize the truncated vector (per the MRL recipe — the prefix of a
/// unit vector is not itself unit-norm).
fn truncate_and_renormalize(full: &[f32]) -> Vec<f32> {
    let take = full.len().min(EMBEDDING_DIM);
    let mut v = full[..take].to_vec();
    let norm = v
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x = ((*x as f64) / norm) as f32;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_renormalizes_to_unit_norm() {
        let full: Vec<f32> = (0..NATIVE_DIM).map(|i| (i as f32 + 1.0).recip()).collect();
        let v = truncate_and_renormalize(&full);
        assert_eq!(v.len(), EMBEDDING_DIM);
        let norm: f64 = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}");
    }

    #[test]
    fn truncation_of_short_vector_keeps_length() {
        let v = truncate_and_renormalize(&[3.0, 4.0]);
        assert_eq!(v.len(), 2);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn zero_vector_does_not_divide_by_zero() {
        let v = truncate_and_renormalize(&[0.0; NATIVE_DIM]);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn prefixes_match_model_card() {
        // The exact strings are part of the model contract; a silent edit
        // would corrupt the whole index. Pin them.
        assert_eq!(QUERY_PREFIX, "task: search result | query: ");
        assert_eq!(DOC_PREFIX, "title: none | text: ");
    }
}
