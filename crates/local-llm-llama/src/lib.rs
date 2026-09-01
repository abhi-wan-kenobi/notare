//! `llama.cpp` text-generation engine for the embedded local LLM server.
//!
//! Deliberately separate from `transcribe-voxtral-llama` (which uses
//! `llama-cpp-2`'s `mtmd` feature for audio-conditioned generation): this
//! crate is plain chat-completion, no multimodal input, and additionally
//! enables the `llguidance` feature for grammar-constrained (JSON-schema)
//! decoding. Both crates share one process-wide `LlamaBackend` via
//! `hypr-llama-cpp-backend`, so they can be active in the same process.
mod model;

pub use model::{
    ChatMessage, FinishReason, GenerateOutcome, GenerateRequest, LlamaLlmModel, LlmError,
};
