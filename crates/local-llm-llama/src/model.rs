use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

/// Context window for the embedded chat server. Generous enough for a
/// multi-turn chat/action-items prompt without needing sliding-window
/// eviction (out of scope for a first cut — a too-long prompt fails closed
/// via `LlmError::PromptTooLong` instead of silently truncating).
const N_CTX: u32 = 8192;
const N_BATCH: u32 = 2048;
/// `response.max_tokens` default when the caller doesn't specify one.
const DEFAULT_MAX_TOKENS: usize = 1024;
/// llama.cpp's own sentinel for "pick a random seed" (`LLAMA_DEFAULT_SEED`).
const RANDOM_SEED: u32 = 0xFFFF_FFFF;

#[derive(thiserror::Error, Debug)]
pub enum LlmError {
    #[error("llama.cpp backend init failed: {0}")]
    Backend(String),
    #[error("failed to load model: {0}")]
    ModelLoad(String),
    #[error("failed to create llama context: {0}")]
    ContextCreate(String),
    #[error("chat template error: {0}")]
    ChatTemplate(String),
    #[error("tokenize error: {0}")]
    Tokenize(String),
    #[error("invalid grammar: {0}")]
    Grammar(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("detokenize error: {0}")]
    Detokenize(String),
    #[error("batch build error: {0}")]
    Batch(String),
    #[error("prompt has {0} tokens, which exceeds the {1}-token context window")]
    PromptTooLong(usize, u32),
}

/// Whether to offload to GPU. Only meaningful when this crate is built with
/// `cuda` or `vulkan`; CPU-only builds (the default) always run on CPU.
const fn use_gpu() -> bool {
    cfg!(feature = "cuda") || cfg!(feature = "vulkan")
}

pub struct LlamaLlmModel {
    model: LlamaModel,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct GenerateRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<usize>,
    /// `None` and `Some(0.0)` both select greedy (deterministic) decoding.
    pub temperature: Option<f32>,
    /// Raw JSON Schema text. When set, decoding is grammar-constrained via
    /// llama.cpp's `llguidance` sampler, so the emitted text is guaranteed
    /// to be schema-valid JSON (barring truncation at `max_tokens`) rather
    /// than merely likely to be.
    pub json_schema: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// Hit an end-of-generation token, or the streaming caller stopped early.
    Stop,
    /// Hit `max_tokens` before the model emitted an end-of-generation token.
    Length,
}

#[derive(Debug, Clone)]
pub struct GenerateOutcome {
    pub text: String,
    pub finish_reason: FinishReason,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

impl LlamaLlmModel {
    pub fn load<P: AsRef<Path>>(model_path: P) -> Result<Self, LlmError> {
        let model_path = model_path.as_ref();
        let backend =
            hypr_llama_cpp_backend::backend().map_err(|e| LlmError::Backend(e.to_string()))?;

        let model_params =
            LlamaModelParams::default().with_n_gpu_layers(if use_gpu() { u32::MAX } else { 0 });

        let model = LlamaModel::load_from_file(backend, model_path, &model_params)
            .map_err(|e| LlmError::ModelLoad(e.to_string()))?;

        tracing::info!(
            model = %model_path.display(),
            gpu = use_gpu(),
            "local_llm_model_loaded"
        );

        Ok(Self { model })
    }

    /// Run one chat completion. `on_token` is called with each generated
    /// text piece as it is produced (for SSE streaming); returning `false`
    /// stops generation early (e.g. a disconnected client). Generation also
    /// stops at an end-of-generation token or `request.max_tokens`.
    ///
    /// A fresh `LlamaContext` per call (no cross-request KV cache reuse) —
    /// same tradeoff `transcribe-voxtral-llama` makes: simplicity over
    /// prompt-cache reuse, revisit if latency demands it.
    pub fn generate(
        &self,
        request: &GenerateRequest,
        mut on_token: impl FnMut(&str) -> bool,
    ) -> Result<GenerateOutcome, LlmError> {
        let backend =
            hypr_llama_cpp_backend::backend().map_err(|e| LlmError::Backend(e.to_string()))?;

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(N_BATCH)
            .with_n_ubatch(N_BATCH)
            .with_n_threads(n_threads)
            .with_n_threads_batch(n_threads);

        let mut llama_ctx: LlamaContext<'_> = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|e| LlmError::ContextCreate(e.to_string()))?;

        let messages: Vec<LlamaChatMessage> = request
            .messages
            .iter()
            .map(|m| LlamaChatMessage::new(m.role.clone(), m.content.clone()))
            .collect::<Result<_, _>>()
            .map_err(|e| LlmError::ChatTemplate(e.to_string()))?;

        let template = self
            .model
            .chat_template(None)
            .map_err(|e| LlmError::ChatTemplate(e.to_string()))?;
        let mut prompt = self
            .model
            .apply_chat_template(&template, &messages, true)
            .map_err(|e| LlmError::ChatTemplate(e.to_string()))?;

        if request.json_schema.is_some() {
            // Empirically required for HyprLLM (a Qwen3-architecture model,
            // confirmed via its GGUF `general.architecture` metadata):
            // Qwen3's template gives the model room to open a `<think>...`
            // reasoning block before its answer. Grammar-constrained
            // decoding forces JSON-object syntax from the very first
            // generated token, so if the model still tries to think first,
            // the very first tokens (`<`, `t`, `h`, ...) don't satisfy the
            // JSON grammar; llguidance's matcher then desyncs and — this was
            // observed directly against the real model — *stops enforcing
            // the grammar for the rest of the generation* rather than
            // failing closed, which would have silently broken Requirement
            // 3's guarantee. Appending an already-closed, empty `<think>`
            // block to the prompt is Qwen3's own documented mechanism for
            // skipping reasoning (the model sees its own turn as having
            // already "thought" and answers directly), so the grammar never
            // has to constrain over a reasoning preamble in the first place.
            prompt.push_str("<think>\n\n</think>\n\n");
        }

        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| LlmError::Tokenize(e.to_string()))?;

        if tokens.len() as u32 >= N_CTX {
            return Err(LlmError::PromptTooLong(tokens.len(), N_CTX));
        }

        let mut n_past = 0i32;
        for chunk in tokens.chunks(N_BATCH as usize) {
            let mut batch = LlamaBatch::new(chunk.len(), 1);
            let last_idx = chunk.len() - 1;
            for (i, token) in chunk.iter().enumerate() {
                let is_last_chunk = n_past as usize + chunk.len() == tokens.len();
                batch
                    .add(
                        *token,
                        n_past + i as i32,
                        &[0],
                        is_last_chunk && i == last_idx,
                    )
                    .map_err(|e| LlmError::Batch(e.to_string()))?;
            }
            llama_ctx
                .decode(&mut batch)
                .map_err(|e| LlmError::Decode(e.to_string()))?;
            n_past += chunk.len() as i32;
        }

        let mut sampler = self.build_sampler(request)?;

        let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut text = String::new();
        let mut finish_reason = FinishReason::Length;
        let mut completion_tokens = 0usize;

        for _ in 0..max_tokens {
            let token = sampler.sample(&llama_ctx, -1);
            sampler.accept(token);

            if self.model.is_eog_token(token) {
                finish_reason = FinishReason::Stop;
                break;
            }

            let piece = self
                .model
                .token_to_piece(token, &mut decoder, false, None)
                .map_err(|e| LlmError::Detokenize(e.to_string()))?;
            text.push_str(&piece);
            completion_tokens += 1;

            if !on_token(&piece) {
                finish_reason = FinishReason::Stop;
                break;
            }

            // A caller-supplied `max_tokens` can exceed what's left of the
            // context window (the prompt already used part of it). Stop
            // cleanly instead of decoding into a full KV cache, which
            // `llama_ctx.decode` would otherwise reject as `LlmError::Decode`
            // partway through an otherwise-successful response.
            if n_past + 1 >= N_CTX as i32 {
                finish_reason = FinishReason::Length;
                break;
            }

            let mut batch = LlamaBatch::new(1, 1);
            batch
                .add(token, n_past, &[0], true)
                .map_err(|e| LlmError::Batch(e.to_string()))?;
            llama_ctx
                .decode(&mut batch)
                .map_err(|e| LlmError::Decode(e.to_string()))?;
            n_past += 1;
        }

        Ok(GenerateOutcome {
            text,
            finish_reason,
            prompt_tokens: tokens.len(),
            completion_tokens,
        })
    }

    /// Build the sampler chain: an optional `llguidance` grammar stage
    /// (masks disallowed tokens) followed by the selection stage (greedy, or
    /// temperature + distribution sampling). The grammar stage runs first so
    /// it constrains what the selection stage is allowed to pick from.
    fn build_sampler(&self, request: &GenerateRequest) -> Result<LlamaSampler, LlmError> {
        let selection = match request.temperature {
            Some(t) if t > 0.0 => {
                LlamaSampler::chain_simple([LlamaSampler::temp(t), LlamaSampler::dist(RANDOM_SEED)])
            }
            _ => LlamaSampler::greedy(),
        };

        match &request.json_schema {
            Some(schema) => {
                let grammar = LlamaSampler::llguidance(&self.model, "json_schema", schema)
                    .map_err(|e| LlmError::Grammar(e.to_string()))?;
                Ok(LlamaSampler::chain_simple([grammar, selection]))
            }
            None => Ok(selection),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_too_long_error_names_both_numbers() {
        let err = LlmError::PromptTooLong(9000, N_CTX);
        let msg = err.to_string();
        assert!(msg.contains("9000"));
        assert!(msg.contains(&N_CTX.to_string()));
    }

    #[test]
    fn finish_reason_is_copy_and_comparable() {
        let a = FinishReason::Stop;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(FinishReason::Stop, FinishReason::Length);
    }

    #[test]
    fn use_gpu_is_false_on_a_plain_cpu_build() {
        // This crate's own test binary is built with this test file's
        // default features (no `cuda`/`vulkan`) unless explicitly enabled.
        assert_eq!(use_gpu(), cfg!(any(feature = "cuda", feature = "vulkan")));
    }
}
