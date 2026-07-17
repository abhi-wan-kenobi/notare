// Runtime: llama.cpp's `libmtmd` multimodal/audio path via the `llama-cpp-2`
// crate's `mtmd` feature (crates.io v0.1.151, utilityai/llama-cpp-rs — the
// binding already exposes `MtmdContext`/`MtmdBitmap` audio loading and an
// `eval_chunks` helper that internally calls `mtmd_encode()` +
// `mtmd_get_output_embd()` + `llama_decode()`; no custom FFI shim needed).
// See the research memo (issue #16 Phase A) for why this beat mistral.rs/
// candle (no Vulkan/AMD backend at all), ONNX Runtime (no vetted native
// runtime for this export) and a vLLM sidecar (breaks the single-binary
// design, needs 16GB+ VRAM for the realtime variant).

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::OnceLock;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::mtmd::{MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputText};
use llama_cpp_2::sampling::LlamaSampler;

/// GGUF text-decoder weight file name, matching
/// `voxtral_llama_model::VoxtralLlamaModel::Mini3bQ4KM::weight_file()`.
/// Duplicated (not imported) on purpose: mirrors how `parakeet-onnx`'s
/// `model.rs` hardcodes its own fixed ONNX file names independently of
/// `parakeet-onnx-model`, keeping the engine crate decoupled from the
/// catalog crate.
pub const WEIGHT_FILE: &str = "Voxtral-Mini-3B-2507-Q4_K_M.gguf";
/// `mmproj` audio-encoder file name, matching
/// `VoxtralLlamaModel::Mini3bQ4KM::mmproj_file()`.
pub const MMPROJ_FILE: &str = "mmproj-Voxtral-Mini-3B-2507-Q8_0.gguf";

/// Context window. libmtmd's audio path is fixed 30s-chunk batch-only today
/// (ggml-org/llama.cpp#20914); 25s of audio plus the chat-template prompt
/// and generated transcript comfortably fits well under this.
const N_CTX: u32 = 8192;
const N_BATCH: u32 = 2048;
/// Hard ceiling on generated tokens per chunk. Generous headroom over what
/// 25s of normal-paced speech decodes to; a runaway generation still
/// terminates here instead of hanging.
const MAX_PREDICT_TOKENS: usize = 768;

#[derive(thiserror::Error, Debug)]
pub enum VoxtralError {
    #[error("llama.cpp backend init failed: {0}")]
    Backend(String),
    #[error("failed to load model: {0}")]
    ModelLoad(String),
    #[error("failed to create llama context: {0}")]
    ContextCreate(String),
    #[error("failed to init mtmd context: {0}")]
    MtmdInit(String),
    #[error("failed to build audio bitmap: {0}")]
    Bitmap(String),
    #[error("chat template error: {0}")]
    ChatTemplate(String),
    #[error("tokenize error: {0}")]
    Tokenize(String),
    #[error("audio eval error: {0}")]
    Eval(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("detokenize error: {0}")]
    Detokenize(String),
    #[error("batch build error: {0}")]
    Batch(String),
    #[error("model mutex poisoned")]
    Poisoned,
}

/// The llama backend can only be initialized once per process
/// (`LlamaBackend::init` errors on a second call) — a single `OnceLock`
/// covers every `VoxtralModel` loaded in this process, including in tests.
fn backend() -> Result<&'static LlamaBackend, VoxtralError> {
    static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| VoxtralError::Backend(e.clone()))
}

/// Whether to offload the LLM decoder + mtmd audio encoder to GPU. Only
/// meaningful when this crate is built with the `cuda` feature; CPU-only
/// builds (the default, and the only supported path on non-CUDA machines
/// per issue #16 Phase A's decision) always run on CPU. Vulkan is
/// deliberately never wired here (heap corruption with mtmd on RDNA2,
/// ggml-org/llama.cpp#22128).
const fn use_gpu() -> bool {
    cfg!(feature = "cuda")
}

pub struct VoxtralModel {
    model: LlamaModel,
    mtmd_ctx: MtmdContext,
}

impl VoxtralModel {
    /// `model_dir` is the model *directory* holding [`WEIGHT_FILE`] and
    /// [`MMPROJ_FILE`] (same "directory of fixed-name files" shape as
    /// `parakeet_onnx::ParakeetModel::new`).
    pub fn new<P: AsRef<Path>>(model_dir: P) -> Result<Self, VoxtralError> {
        let model_dir = model_dir.as_ref();
        let weight_path = model_dir.join(WEIGHT_FILE);
        let mmproj_path = model_dir.join(MMPROJ_FILE);

        let backend = backend()?;

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(if use_gpu() { u32::MAX } else { 0 });

        let model = LlamaModel::load_from_file(backend, &weight_path, &model_params)
            .map_err(|e| VoxtralError::ModelLoad(e.to_string()))?;

        let mmproj_path_str = mmproj_path.to_string_lossy().into_owned();
        let mtmd_params = MtmdContextParams {
            use_gpu: use_gpu(),
            print_timings: false,
            n_threads,
            ..Default::default()
        };
        let mtmd_ctx = MtmdContext::init_from_file(&mmproj_path_str, &model, &mtmd_params)
            .map_err(|e| VoxtralError::MtmdInit(format!("{e:?}")))?;

        tracing::info!(
            weight = %weight_path.display(),
            mmproj = %mmproj_path.display(),
            gpu = use_gpu(),
            support_audio = mtmd_ctx.support_audio(),
            "voxtral_llama_model_loaded"
        );

        Ok(Self { model, mtmd_ctx })
    }

    /// Transcribe one chunk of 16kHz mono f32 samples. Each call gets a
    /// fresh `LlamaContext` (fresh KV cache): libmtmd's audio path is
    /// batch/static-file only today (no cross-chunk streaming state to
    /// preserve), so this matches upstream's own current design instead of
    /// fighting it.
    pub fn transcribe_samples(&mut self, samples: &[f32]) -> Result<String, VoxtralError> {
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);

        let backend = backend()?;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(N_BATCH)
            .with_n_ubatch(N_BATCH)
            .with_n_threads(n_threads)
            .with_n_threads_batch(n_threads);

        let mut llama_ctx: LlamaContext<'_> = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|e| VoxtralError::ContextCreate(e.to_string()))?;

        let bitmap = MtmdBitmap::from_audio_data(samples)
            .map_err(|e| VoxtralError::Bitmap(format!("{e:?}")))?;

        // Mistral's own `apply_transcription_request` (mistral-common) hits a
        // dedicated non-chat transcription format that isn't reachable
        // through llama.cpp's chat-template plumbing. Going through
        // `apply_chat_template` with just the audio marker and no
        // instruction — verified empirically against this exact model —
        // makes Voxtral treat the turn as an open-ended "understanding"
        // question and answer *about* the audio (a restructured, markdown-
        // formatted essay) instead of transcribing it. An explicit verbatim
        // instruction alongside the marker reliably gets literal ASR
        // output instead.
        let marker = llama_cpp_2::mtmd::mtmd_default_marker();
        let instruction =
            format!("{marker}Repeat exactly, word for word, what is said in the audio above. Output only the verbatim transcript: no summary, no commentary, no headings, no markdown, no added punctuation beyond what is spoken.");
        let messages = [LlamaChatMessage::new("user".to_string(), instruction)
            .map_err(|e| VoxtralError::ChatTemplate(e.to_string()))?];
        let template = self
            .model
            .chat_template(None)
            .map_err(|e| VoxtralError::ChatTemplate(e.to_string()))?;
        let prompt = self
            .model
            .apply_chat_template(&template, &messages, true)
            .map_err(|e| VoxtralError::ChatTemplate(e.to_string()))?;

        let input_text = MtmdInputText {
            text: prompt,
            add_special: true,
            parse_special: true,
        };
        let chunks = self
            .mtmd_ctx
            .tokenize(input_text, &[&bitmap])
            .map_err(|e| VoxtralError::Tokenize(format!("{e:?}")))?;

        let mut n_past = chunks
            .eval_chunks(&self.mtmd_ctx, &llama_ctx, 0, 0, N_BATCH as i32, true)
            .map_err(|e| VoxtralError::Eval(format!("{e:?}")))?;

        let mut sampler = LlamaSampler::greedy();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut output = String::new();

        for _ in 0..MAX_PREDICT_TOKENS {
            let token = sampler.sample(&llama_ctx, -1);

            if self.model.is_eog_token(token) {
                break;
            }

            let piece = self
                .model
                .token_to_piece(token, &mut decoder, false, None)
                .map_err(|e| VoxtralError::Detokenize(e.to_string()))?;
            output.push_str(&piece);

            let mut batch = LlamaBatch::new(1, 1);
            batch
                .add(token, n_past, &[0], true)
                .map_err(|e| VoxtralError::Batch(e.to_string()))?;
            llama_ctx
                .decode(&mut batch)
                .map_err(|e| VoxtralError::Decode(e.to_string()))?;
            n_past += 1;
        }

        Ok(output.trim().to_string())
    }
}
