//! Process-wide `llama.cpp` backend singleton.
//!
//! `llama-cpp-2`'s `LlamaBackend::init()` guards itself with an internal
//! `AtomicBool` that is `static` inside the `llama-cpp-2` crate itself — it is
//! genuinely process-global, not per-callsite, because Cargo unifies the
//! dependency to one compiled copy. Calling `init()` a second time from a
//! second, independent `OnceLock` (e.g. one owned by the Voxtral STT engine
//! and another owned by the local LLM server) does not panic, but the second
//! call returns `Err(BackendAlreadyInitialized)` permanently, because the
//! first `LlamaBackend` instance is never dropped for the life of the
//! process.
//!
//! Every crate in this workspace that touches `llama-cpp-2` directly (Voxtral
//! STT's `transcribe-voxtral-llama`, the embedded LLM server's
//! `local-llm-llama`) must obtain the backend through this single shared
//! `OnceLock` instead of holding its own, so both can be active in the same
//! process at once.
use std::sync::OnceLock;

use llama_cpp_2::llama_backend::LlamaBackend;

#[derive(thiserror::Error, Debug, Clone)]
#[error("llama.cpp backend init failed: {0}")]
pub struct BackendError(String);

/// The shared backend. Initializes on first call from any consumer crate;
/// every subsequent call (from the same or a different consumer) returns the
/// same instance.
pub fn backend() -> Result<&'static LlamaBackend, BackendError> {
    static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| BackendError(e.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_calls_share_one_backend() {
        let a = backend().expect("first init succeeds");
        let b = backend().expect("second call reuses the cached instance");
        assert!(std::ptr::eq(a, b));
    }
}
