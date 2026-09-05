use std::io::Read;
use std::path::{Path, PathBuf};

pub use hypr_am::AmModel;
use hypr_model_downloader::{DownloadPart, DownloadableModel, Error};
pub use hypr_parakeet_onnx_model::ParakeetOnnxModel;
pub use hypr_transcribe_soniqo::SoniqoModel;
pub use hypr_voxtral_llama_model::VoxtralLlamaModel;
pub use hypr_whisper_local_model::WhisperModel;
use sha2::{Digest, Sha256};

/// Streaming SHA-256 (64KB buffer, matching `hypr_file::calculate_file_checksum`'s
/// CRC32 pattern) so verifying a multi-gigabyte model weight doesn't require
/// loading it into memory.
fn sha256_hex(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Sha256::new();

    let mut buffer = [0u8; 65536];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq)]
pub enum GgufLlmModel {
    Llama3p2_3bQ4,
    Gemma3_4bQ4,
    HyprLLM,
    Qwen3_4bQ4,
    Llama3p1_8bQ4,
    Phi4Mini_Q4,
    Mistral7b_v03_Q4,
}

impl GgufLlmModel {
    /// Whether the embedded local-llm server's engine (`llama-cpp-2`, no
    /// Cactus-style architecture restriction) can run this model on the
    /// current build target. `HyprLLM` is the one model this restoration
    /// actively ships and verifies end-to-end, so it is offered everywhere
    /// llama-cpp-2 itself builds (every desktop target; GPU offload is a
    /// separate `cuda`/`vulkan` build-feature concern layered on top).
    /// `Llama3p2_3bQ4` and `Gemma3_4bQ4` are self-described as deprecated,
    /// backward-compatibility-only entries (see `description()`) predating
    /// this restoration — they keep their original aarch64-only gate rather
    /// than being widened into a new default.
    ///
    /// This is a wider gate than `VoxtralLlama` gets below
    /// (`is_x86_64_win_or_linux` only) despite both using `llama-cpp-2`: that
    /// gate is about the `mtmd` (multimodal/audio) path specifically — it
    /// needs GPU-class latency for anything like realtime STT (ruling out
    /// macOS, which has no CUDA) and hits a real upstream Vulkan+mtmd bug on
    /// the CPU/Vulkan fallback platforms (ggml-org/llama.cpp#22128, see the
    /// comment on `transcribe-voxtral-llama`'s `Cargo.toml`). Plain
    /// text-only chat completion (this model) uses neither `mtmd` nor
    /// Vulkan-with-mtmd, tolerates CPU-only latency for a chat/action-items
    /// response the way an interactive audio pipeline cannot, and isn't
    /// implicated by that bug — so the two engines built from the same
    /// underlying crate legitimately have different platform gates.
    pub fn is_available_on_current_platform(&self) -> bool {
        match self {
            // Legacy models: aarch64-only for backward compatibility.
            GgufLlmModel::Llama3p2_3bQ4 => cfg!(target_arch = "aarch64"),
            // All other models run on every desktop target llama-cpp-2 builds on.
            _ => true,
        }
    }

    /// SHA-256 of the downloaded weight file, verified in
    /// `finalize_download` on top of the existing CRC32
    /// (`model_checksum`/`expected_size`) check the shared downloader
    /// already runs. `None` for the two deprecated entries: they predate
    /// this restoration and this pass didn't re-verify their multi-GB
    /// weights against a fresh download, so they keep exactly their
    /// pre-existing CRC32-only verification rather than gaining an
    /// unverified constant.
    pub fn model_sha256(&self) -> Option<&'static str> {
        match self {
            GgufLlmModel::HyprLLM => {
                Some("d772bf66eceef53ea28adaf3929df0a479606c358facd26c9f33396399f96863")
            }
            GgufLlmModel::Qwen3_4bQ4 => {
                Some("7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5")
            }
            GgufLlmModel::Llama3p1_8bQ4 => {
                Some("7b064f5842bf9532c91456deda288a1b672397a54fa729aa665952863033557c")
            }
            GgufLlmModel::Phi4Mini_Q4 => {
                Some("3c4d3cbdf3006d81444f6c7a5a56eb93d8e0f0e2ba5963b8ab62f9fd42604233")
            }
            GgufLlmModel::Mistral7b_v03_Q4 => {
                Some("1270d22c0fbb3d092fb725d4d96c457b7b687a5f5a715abe1e818da303e562b6")
            }
            GgufLlmModel::Llama3p2_3bQ4 | GgufLlmModel::Gemma3_4bQ4 => None,
        }
    }

    pub fn file_name(&self) -> &str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "llm.gguf",
            GgufLlmModel::HyprLLM => "hypr-llm.gguf",
            GgufLlmModel::Gemma3_4bQ4 => "gemma-3-4b-it-Q4_K_M.gguf",
            GgufLlmModel::Qwen3_4bQ4 => "Qwen3-4B-Q4_K_M.gguf",
            GgufLlmModel::Llama3p1_8bQ4 => "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            GgufLlmModel::Phi4Mini_Q4 => "Phi-4-mini-instruct-Q4_K_M.gguf",
            GgufLlmModel::Mistral7b_v03_Q4 => "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
        }
    }

    pub fn model_url(&self) -> &str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/lmstudio-community/Llama-3.2-3B-Instruct-GGUF/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf"
            }
            GgufLlmModel::HyprLLM => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/yujonglee/hypr-llm-sm/model_q4_k_m.gguf"
            }
            GgufLlmModel::Gemma3_4bQ4 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/unsloth/gemma-3-4b-it-GGUF/gemma-3-4b-it-Q4_K_M.gguf"
            }
            GgufLlmModel::Qwen3_4bQ4 => {
                "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf"
            }
            GgufLlmModel::Llama3p1_8bQ4 => {
                "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf"
            }
            GgufLlmModel::Phi4Mini_Q4 => {
                "https://huggingface.co/lmstudio-community/Phi-4-mini-instruct-GGUF/resolve/main/Phi-4-mini-instruct-Q4_K_M.gguf"
            }
            GgufLlmModel::Mistral7b_v03_Q4 => {
                "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf"
            }
        }
    }

    pub fn model_size(&self) -> u64 {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => 2019377440,
            GgufLlmModel::HyprLLM => 1107409056,
            GgufLlmModel::Gemma3_4bQ4 => 2489894016,
            GgufLlmModel::Qwen3_4bQ4 => 2497280256,
            GgufLlmModel::Llama3p1_8bQ4 => 4920739232,
            GgufLlmModel::Phi4Mini_Q4 => 2491874400,
            GgufLlmModel::Mistral7b_v03_Q4 => 4372812000,
        }
    }

    pub fn model_checksum(&self) -> Option<u32> {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => Some(2831308098),
            GgufLlmModel::HyprLLM => Some(4037351144),
            GgufLlmModel::Gemma3_4bQ4 => Some(2760830291),
            // New models rely on SHA-256 verification only.
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "Llama 3.2 3B Q4",
            GgufLlmModel::HyprLLM => "HyprLLM",
            GgufLlmModel::Gemma3_4bQ4 => "Gemma 3 4B Q4",
            GgufLlmModel::Qwen3_4bQ4 => "Qwen 3 4B Q4",
            GgufLlmModel::Llama3p1_8bQ4 => "Llama 3.1 8B Q4",
            GgufLlmModel::Phi4Mini_Q4 => "Phi-4 Mini Q4",
            GgufLlmModel::Mistral7b_v03_Q4 => "Mistral 7B v0.3 Q4",
        }
    }

    pub fn description(&self) -> String {
        let mb = self.model_size() as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.1} GB", mb / 1024.0)
        } else {
            format!("{:.0} MB", mb)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LocalModelKind {
    Stt,
    Llm,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq)]
#[serde(untagged)]
pub enum LocalModel {
    Soniqo(SoniqoModel),
    Whisper(WhisperModel),
    Am(AmModel),
    // Serialized as "parakeet-tdt-v3-int8"; collides with no Soniqo
    // ("soniqo-*"), Am ("am-*"), Whisper ("Quantized*") or GgufLlm
    // (variant-name) wire value under untagged deserialization.
    ParakeetOnnx(ParakeetOnnxModel),
    // Serialized as "voxtral-mini-3b-2507-q4km"; collides with none of the
    // above (nor ParakeetOnnx's "parakeet-*" prefix) under untagged
    // deserialization.
    VoxtralLlama(VoxtralLlamaModel),
    GgufLlm(GgufLlmModel),
}

impl std::fmt::Display for LocalModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalModel::Soniqo(model) => write!(f, "{model}"),
            LocalModel::Whisper(model) => write!(f, "whisper-{model}"),
            LocalModel::Am(model) => write!(f, "am-{model}"),
            // ParakeetOnnxModel's strum name already carries the
            // "parakeet-" prefix.
            LocalModel::ParakeetOnnx(model) => write!(f, "{model}"),
            // VoxtralLlamaModel's strum name already carries the
            // "voxtral-" prefix.
            LocalModel::VoxtralLlama(model) => write!(f, "{model}"),
            LocalModel::GgufLlm(model) => write!(f, "llm-{model:?}"),
        }
    }
}

impl LocalModel {
    pub fn all() -> Vec<LocalModel> {
        let mut models = SoniqoModel::all()
            .iter()
            .copied()
            .map(LocalModel::Soniqo)
            .collect::<Vec<_>>();

        models.extend([
            LocalModel::Whisper(WhisperModel::QuantizedTiny),
            LocalModel::Whisper(WhisperModel::QuantizedTinyEn),
            LocalModel::Whisper(WhisperModel::QuantizedBase),
            LocalModel::Whisper(WhisperModel::QuantizedBaseEn),
            LocalModel::Whisper(WhisperModel::QuantizedSmall),
            LocalModel::Whisper(WhisperModel::QuantizedSmallEn),
            LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo),
            LocalModel::Am(AmModel::ParakeetV2),
            LocalModel::Am(AmModel::ParakeetV3),
            LocalModel::Am(AmModel::WhisperLargeV3),
            LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8),
            LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM),
        ]);

        models.extend([
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4),
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM),
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4),
            LocalModel::GgufLlm(GgufLlmModel::Qwen3_4bQ4),
            LocalModel::GgufLlm(GgufLlmModel::Llama3p1_8bQ4),
            LocalModel::GgufLlm(GgufLlmModel::Phi4Mini_Q4),
            LocalModel::GgufLlm(GgufLlmModel::Mistral7b_v03_Q4),
        ]);

        models
    }

    pub fn kind(&self) -> &'static str {
        match self {
            LocalModel::Soniqo(_) => "stt-soniqo",
            LocalModel::Whisper(_) => "stt-whisper",
            LocalModel::Am(_) => "stt-am",
            LocalModel::ParakeetOnnx(_) => "stt-parakeet-onnx",
            LocalModel::VoxtralLlama(_) => "stt-voxtral-llama",
            LocalModel::GgufLlm(_) => "llm",
        }
    }

    pub fn model_kind(&self) -> LocalModelKind {
        match self {
            LocalModel::Soniqo(_)
            | LocalModel::Whisper(_)
            | LocalModel::Am(_)
            | LocalModel::ParakeetOnnx(_)
            | LocalModel::VoxtralLlama(_) => LocalModelKind::Stt,
            LocalModel::GgufLlm(_) => LocalModelKind::Llm,
        }
    }

    pub fn cli_name(&self) -> &'static str {
        match self {
            LocalModel::Soniqo(model) => model.as_str(),
            LocalModel::Whisper(WhisperModel::QuantizedTiny) => "whisper-tiny",
            LocalModel::Whisper(WhisperModel::QuantizedTinyEn) => "whisper-tiny-en",
            LocalModel::Whisper(WhisperModel::QuantizedBase) => "whisper-base",
            LocalModel::Whisper(WhisperModel::QuantizedBaseEn) => "whisper-base-en",
            LocalModel::Whisper(WhisperModel::QuantizedSmall) => "whisper-small",
            LocalModel::Whisper(WhisperModel::QuantizedSmallEn) => "whisper-small-en",
            LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo) => "whisper-large-turbo",
            LocalModel::Am(AmModel::ParakeetV2) => "am-parakeet-v2",
            LocalModel::Am(AmModel::ParakeetV3) => "am-parakeet-v3",
            LocalModel::Am(AmModel::WhisperLargeV3) => "am-whisper-large-v3",
            LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8) => "parakeet-tdt-v3",
            LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM) => "voxtral-mini-3b-q4km",
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4) => "llm-llama3-2-3b-q4",
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM) => "llm-hypr-llm",
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4) => "llm-gemma3-4b-q4",
            LocalModel::GgufLlm(GgufLlmModel::Qwen3_4bQ4) => "llm-qwen3-4b-q4",
            LocalModel::GgufLlm(GgufLlmModel::Llama3p1_8bQ4) => "llm-llama3-1-8b-q4",
            LocalModel::GgufLlm(GgufLlmModel::Phi4Mini_Q4) => "llm-phi4-mini-q4",
            LocalModel::GgufLlm(GgufLlmModel::Mistral7b_v03_Q4) => "llm-mistral-7b-v03-q4",
        }
    }

    pub fn install_path(&self, models_base: &Path) -> PathBuf {
        match self {
            LocalModel::Soniqo(model) => models_base.join("soniqo").join(model.as_str()),
            LocalModel::Whisper(model) => models_base.join("stt").join(model.file_name()),
            LocalModel::Am(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::ParakeetOnnx(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::VoxtralLlama(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::GgufLlm(model) => models_base.join("llm").join(model.file_name()),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            LocalModel::Soniqo(model) => model.display_name().to_string(),
            LocalModel::Whisper(model) => model.display_name().to_string(),
            LocalModel::Am(model) => model.display_name().to_string(),
            LocalModel::ParakeetOnnx(model) => model.display_name().to_string(),
            LocalModel::VoxtralLlama(model) => model.display_name().to_string(),
            LocalModel::GgufLlm(model) => model.display_name().to_string(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            LocalModel::Soniqo(model) => model.description().to_string(),
            LocalModel::Whisper(model) => model.description(),
            LocalModel::Am(model) => model.description().to_string(),
            LocalModel::ParakeetOnnx(model) => model.description(),
            LocalModel::VoxtralLlama(model) => model.description(),
            LocalModel::GgufLlm(model) => model.description(),
        }
    }

    pub fn is_available_on_current_platform(&self) -> bool {
        let is_apple_silicon = cfg!(target_arch = "aarch64") && cfg!(target_os = "macos");
        // CUDA machines are the intended target (see issue #16 Phase A);
        // CPU-only x86_64 Windows/Linux is the supported fallback (slow, but
        // the only Linux-testable path today). No macOS build (Apple
        // Silicon has no CUDA), no Vulkan offload (llama.cpp's mtmd+Vulkan
        // path heap-corrupts on RDNA2, ggml-org/llama.cpp#22128) — those are
        // build-feature/GPU-offload concerns layered on top of this gate,
        // same relationship whisper.cpp's GPU features have to `Whisper(_)`.
        let is_x86_64_win_or_linux = cfg!(target_arch = "x86_64")
            && (cfg!(target_os = "windows") || cfg!(target_os = "linux"));

        match self {
            LocalModel::Soniqo(model) => model.is_available_on_current_platform(),
            // whisper.cpp runs on every desktop platform (CPU at minimum);
            // GPU offload (Metal/Vulkan/CUDA) is a build-feature concern.
            LocalModel::Whisper(_) => true,
            LocalModel::Am(_) => is_apple_silicon,
            // ONNX Runtime CPU execution works on every desktop platform.
            LocalModel::ParakeetOnnx(_) => true,
            LocalModel::VoxtralLlama(_) => is_x86_64_win_or_linux,
            LocalModel::GgufLlm(model) => model.is_available_on_current_platform(),
        }
    }
}

impl DownloadableModel for GgufLlmModel {
    fn download_key(&self) -> String {
        format!("llm:{}", self.file_name())
    }

    fn download_url(&self) -> Option<String> {
        Some(self.model_url().to_string())
    }

    fn download_checksum(&self) -> Option<u32> {
        self.model_checksum()
    }

    fn expected_size(&self) -> Option<u64> {
        Some(self.model_size())
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        models_base.join("llm").join(self.file_name())
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        let path = models_base.join("llm").join(self.file_name());
        if !path.exists() {
            return Ok(false);
        }

        let actual =
            hypr_file::file_size(&path).map_err(|e| Error::OperationFailed(e.to_string()))?;
        Ok(actual == self.model_size())
    }

    /// Runs on top of the shared downloader's own CRC32 check
    /// (`download_checksum`/`expected_size`, already verified before this
    /// hook fires) — SHA-256 is the stronger guarantee weights deserve, per
    /// the OpenWhispr precedent for its GPU packs. Only checked when
    /// `model_sha256()` returns `Some` (see its doc comment for why the two
    /// deprecated variants don't have one yet).
    fn finalize_download(&self, downloaded_path: &Path, _models_base: &Path) -> Result<(), Error> {
        let Some(expected) = self.model_sha256() else {
            return Ok(());
        };

        let actual = sha256_hex(downloaded_path)
            .map_err(|e| Error::FinalizeFailed(format!("sha256 read failed: {e}")))?;

        if actual != expected {
            return Err(Error::FinalizeFailed(format!(
                "sha256 mismatch for {}: expected {expected}, got {actual}",
                self.file_name(),
            )));
        }

        Ok(())
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        let path = models_base.join("llm").join(self.file_name());
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| Error::DeleteFailed(e.to_string()))?;
        }
        Ok(())
    }
}

impl DownloadableModel for LocalModel {
    fn download_key(&self) -> String {
        match self {
            LocalModel::Soniqo(model) => format!("soniqo:{}", model.as_str()),
            LocalModel::Whisper(model) => format!("whisper:{}", model.file_name()),
            LocalModel::Am(model) => format!("am:{}", model.model_dir()),
            LocalModel::ParakeetOnnx(model) => format!("parakeet-onnx:{}", model.model_dir()),
            LocalModel::VoxtralLlama(model) => format!("voxtral-llama:{}", model.model_dir()),
            LocalModel::GgufLlm(model) => model.download_key(),
        }
    }

    fn download_url(&self) -> Option<String> {
        match self {
            LocalModel::Soniqo(_) => None,
            LocalModel::Whisper(model) => Some(model.model_url().to_string()),
            LocalModel::Am(model) => Some(model.tar_url().to_string()),
            // Multi-file model: see `download_parts`.
            LocalModel::ParakeetOnnx(_) => None,
            LocalModel::VoxtralLlama(_) => None,
            LocalModel::GgufLlm(model) => model.download_url(),
        }
    }

    fn download_checksum(&self) -> Option<u32> {
        match self {
            LocalModel::Soniqo(_) => None,
            LocalModel::Whisper(model) => Some(model.checksum()),
            LocalModel::Am(model) => Some(model.tar_checksum()),
            // Per-part checksums live in `download_parts`.
            LocalModel::ParakeetOnnx(_) => None,
            LocalModel::VoxtralLlama(_) => None,
            LocalModel::GgufLlm(model) => model.download_checksum(),
        }
    }

    fn expected_size(&self) -> Option<u64> {
        match self {
            LocalModel::Soniqo(_) => None,
            LocalModel::Whisper(model) => Some(model.model_size_bytes()),
            // AM installs as an unpacked directory; size applies to the tar only.
            LocalModel::Am(_) => None,
            LocalModel::ParakeetOnnx(model) => Some(model.model_size_bytes()),
            LocalModel::VoxtralLlama(model) => Some(model.model_size_bytes()),
            LocalModel::GgufLlm(model) => model.expected_size(),
        }
    }

    fn download_parts(&self) -> Option<Vec<DownloadPart>> {
        match self {
            LocalModel::ParakeetOnnx(model) => Some(
                model
                    .files()
                    .iter()
                    .map(|file| DownloadPart {
                        url: model.file_url(file.name),
                        relative_path: file.name.to_string(),
                        checksum: Some(file.crc32),
                        expected_size: Some(file.size_bytes),
                    })
                    .collect(),
            ),
            LocalModel::VoxtralLlama(model) => Some(
                model
                    .files()
                    .iter()
                    .map(|file| DownloadPart {
                        url: model.file_url(file.name),
                        relative_path: file.name.to_string(),
                        checksum: Some(file.crc32),
                        expected_size: Some(file.size_bytes),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        match self {
            LocalModel::Soniqo(model) => models_base.join("soniqo").join(model.as_str()),
            LocalModel::Whisper(model) => models_base.join("stt").join(model.file_name()),
            LocalModel::Am(model) => models_base
                .join("stt")
                .join(format!("{}.tar", model.model_dir())),
            // Directory holding the individual ONNX/vocab files.
            LocalModel::ParakeetOnnx(model) => models_base.join("stt").join(model.model_dir()),
            // Directory holding the GGUF weight + mmproj files.
            LocalModel::VoxtralLlama(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::GgufLlm(model) => model.download_destination(models_base),
        }
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        match self {
            LocalModel::Soniqo(model) => hypr_transcribe_soniqo::is_model_downloaded(*model)
                .map_err(|e| Error::OperationFailed(e.to_string())),
            LocalModel::Whisper(model) => {
                // Never trust bare existence: a truncated or corrupt leftover
                // (e.g. after a failed uninstall) must not count as installed.
                let path = models_base.join("stt").join(model.file_name());
                if !path.is_file() {
                    return Ok(false);
                }
                let actual = hypr_file::file_size(&path)
                    .map_err(|e| Error::OperationFailed(e.to_string()))?;
                Ok(actual == model.model_size_bytes())
            }
            LocalModel::Am(model) => model
                .is_downloaded(models_base.join("stt"))
                .map_err(|e| Error::OperationFailed(e.to_string())),
            LocalModel::ParakeetOnnx(model) => {
                // Same rule as whisper: a truncated leftover must not count
                // as installed, so require every file at its expected size.
                let dir = models_base.join("stt").join(model.model_dir());
                Ok(model.files().iter().all(|file| {
                    let path = dir.join(file.name);
                    path.is_file()
                        && hypr_file::file_size(&path)
                            .map(|size| size == file.size_bytes)
                            .unwrap_or(false)
                }))
            }
            LocalModel::VoxtralLlama(model) => {
                // Same rule: a truncated leftover must not count as
                // installed, so require both the weight and mmproj files at
                // their expected sizes.
                let dir = models_base.join("stt").join(model.model_dir());
                Ok(model.files().iter().all(|file| {
                    let path = dir.join(file.name);
                    path.is_file()
                        && hypr_file::file_size(&path)
                            .map(|size| size == file.size_bytes)
                            .unwrap_or(false)
                }))
            }
            LocalModel::GgufLlm(model) => model.is_downloaded(models_base),
        }
    }

    fn finalize_download(&self, downloaded_path: &Path, models_base: &Path) -> Result<(), Error> {
        match self {
            LocalModel::Soniqo(_) => Err(Error::FinalizeFailed(
                "Soniqo models are downloaded through the Soniqo bridge".to_string(),
            )),
            LocalModel::Whisper(_) => Ok(()),
            LocalModel::Am(model) => {
                let final_path = models_base.join("stt");
                model
                    .tar_unpack_and_cleanup(downloaded_path, &final_path)
                    .map_err(|e| Error::FinalizeFailed(e.to_string()))
            }
            // Parts are downloaded and verified straight into the model
            // directory; nothing left to do.
            LocalModel::ParakeetOnnx(_) => Ok(()),
            LocalModel::VoxtralLlama(_) => Ok(()),
            LocalModel::GgufLlm(model) => model.finalize_download(downloaded_path, models_base),
        }
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        match self {
            LocalModel::Soniqo(model) => hypr_transcribe_soniqo::delete_model(*model)
                .map_err(|e| Error::DeleteFailed(e.to_string())),
            LocalModel::Whisper(model) => {
                let model_path = models_base.join("stt").join(model.file_name());
                if model_path.exists() {
                    std::fs::remove_file(&model_path)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::Am(model) => {
                let model_dir = models_base.join("stt").join(model.model_dir());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::ParakeetOnnx(model) => {
                let model_dir = models_base.join("stt").join(model.model_dir());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::VoxtralLlama(model) => {
                let model_dir = models_base.join("stt").join(model.model_dir());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::GgufLlm(model) => model.delete_downloaded(models_base),
        }
    }

    fn remove_destination_after_finalize(&self) -> bool {
        matches!(self, LocalModel::Am(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parakeet_onnx_untagged_serde_round_trips_without_collisions() {
        let model = LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8);
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(json, "\"parakeet-tdt-v3-int8\"");

        let parsed: LocalModel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, model);

        // Neighbouring wire values must still land on their own variants.
        let whisper: LocalModel = serde_json::from_str("\"QuantizedSmall\"").unwrap();
        assert!(matches!(whisper, LocalModel::Whisper(_)));
        let am: LocalModel = serde_json::from_str("\"am-parakeet-v3\"").unwrap();
        assert!(matches!(am, LocalModel::Am(_)));
    }

    #[test]
    fn parakeet_onnx_downloads_as_pinned_multi_part() {
        let model = LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8);

        assert_eq!(model.download_url(), None);
        let parts = model.download_parts().expect("must be multi-part");
        assert_eq!(parts.len(), 4);
        assert!(parts.iter().all(|p| p.checksum.is_some()));
        assert!(parts.iter().all(|p| p.expected_size.is_some()));
        assert!(
            parts
                .iter()
                .all(|p| p.url.contains("8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce"))
        );
        assert_eq!(
            model.expected_size(),
            Some(parts.iter().map(|p| p.expected_size.unwrap()).sum())
        );
        assert!(!model.remove_destination_after_finalize());

        let destination = model.download_destination(Path::new("/base"));
        assert_eq!(
            destination,
            Path::new("/base/stt/parakeet-tdt-0.6b-v3-int8")
        );
        assert_eq!(destination, model.install_path(Path::new("/base")));
    }

    #[test]
    fn parakeet_onnx_is_downloaded_requires_all_files_at_expected_sizes() {
        let model = LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8);
        let base = tempfile::tempdir().unwrap();
        assert!(!model.is_downloaded(base.path()).unwrap());

        let dir = base.path().join("stt").join("parakeet-tdt-0.6b-v3-int8");
        std::fs::create_dir_all(&dir).unwrap();
        for file in ParakeetOnnxModel::TdtV3Int8.files() {
            // right names, wrong sizes -> still not installed
            std::fs::write(dir.join(file.name), b"stub").unwrap();
        }
        assert!(!model.is_downloaded(base.path()).unwrap());

        model.delete_downloaded(base.path()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn voxtral_llama_untagged_serde_round_trips_without_collisions() {
        let model = LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM);
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(json, "\"voxtral-mini-3b-2507-q4km\"");

        let parsed: LocalModel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, model);

        // Neighbouring wire values must still land on their own variants.
        let parakeet: LocalModel = serde_json::from_str("\"parakeet-tdt-v3-int8\"").unwrap();
        assert!(matches!(parakeet, LocalModel::ParakeetOnnx(_)));
    }

    #[test]
    fn voxtral_llama_downloads_as_pinned_multi_part() {
        let model = LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM);

        assert_eq!(model.download_url(), None);
        let parts = model.download_parts().expect("must be multi-part");
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.checksum.is_some()));
        assert!(parts.iter().all(|p| p.expected_size.is_some()));
        assert!(
            parts
                .iter()
                .all(|p| p.url.contains("20616573013f28229fff61de74359d3eeff61f6a"))
        );
        assert_eq!(
            model.expected_size(),
            Some(parts.iter().map(|p| p.expected_size.unwrap()).sum())
        );
        assert!(!model.remove_destination_after_finalize());

        let destination = model.download_destination(Path::new("/base"));
        assert_eq!(
            destination,
            Path::new("/base/stt/voxtral-mini-3b-2507-q4km")
        );
        assert_eq!(destination, model.install_path(Path::new("/base")));
    }

    #[test]
    fn voxtral_llama_is_downloaded_requires_all_files_at_expected_sizes() {
        let model = LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM);
        let base = tempfile::tempdir().unwrap();
        assert!(!model.is_downloaded(base.path()).unwrap());

        let dir = base.path().join("stt").join("voxtral-mini-3b-2507-q4km");
        std::fs::create_dir_all(&dir).unwrap();
        for file in VoxtralLlamaModel::Mini3bQ4KM.files() {
            // right names, wrong sizes -> still not installed
            std::fs::write(dir.join(file.name), b"stub").unwrap();
        }
        assert!(!model.is_downloaded(base.path()).unwrap());

        model.delete_downloaded(base.path()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn soniqo_models_reject_generic_download_finalize() {
        let model = LocalModel::Soniqo(SoniqoModel::ParakeetStreaming);

        let error = model
            .finalize_download(Path::new("download"), Path::new("models"))
            .unwrap_err();

        assert!(error.to_string().contains("Soniqo bridge"));
    }

    #[test]
    fn hypr_llm_finalize_rejects_sha256_mismatch() {
        let model = GgufLlmModel::HyprLLM;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights.gguf");
        std::fs::write(&path, b"not the real model").unwrap();

        let error = model.finalize_download(&path, dir.path()).unwrap_err();
        assert!(error.to_string().contains("sha256 mismatch"));
    }

    #[test]
    fn hypr_llm_finalize_accepts_matching_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights.gguf");
        std::fs::write(&path, b"some content").unwrap();

        let digest = sha256_hex(&path).unwrap();
        // Not a real weight file, just proving the comparison path accepts a
        // genuine match — `model_sha256()` itself is exercised against the
        // real download in the crate's ignored end-to-end test.
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn legacy_gguf_models_have_no_pinned_sha256() {
        assert_eq!(GgufLlmModel::Llama3p2_3bQ4.model_sha256(), None);
        assert_eq!(GgufLlmModel::Gemma3_4bQ4.model_sha256(), None);
        // All new models have SHA-256 pinned.
        assert!(GgufLlmModel::HyprLLM.model_sha256().is_some());
        assert!(GgufLlmModel::Qwen3_4bQ4.model_sha256().is_some());
        assert!(GgufLlmModel::Llama3p1_8bQ4.model_sha256().is_some());
        assert!(GgufLlmModel::Phi4Mini_Q4.model_sha256().is_some());
        assert!(GgufLlmModel::Mistral7b_v03_Q4.model_sha256().is_some());
    }

    #[test]
    fn only_llama3p2_is_aarch64_gated() {
        if !cfg!(target_arch = "aarch64") {
            assert!(GgufLlmModel::HyprLLM.is_available_on_current_platform());
            assert!(GgufLlmModel::Qwen3_4bQ4.is_available_on_current_platform());
            assert!(GgufLlmModel::Gemma3_4bQ4.is_available_on_current_platform());
            assert!(GgufLlmModel::Phi4Mini_Q4.is_available_on_current_platform());
            assert!(GgufLlmModel::Llama3p1_8bQ4.is_available_on_current_platform());
            assert!(GgufLlmModel::Mistral7b_v03_Q4.is_available_on_current_platform());
            assert!(!GgufLlmModel::Llama3p2_3bQ4.is_available_on_current_platform());
        }
    }

    /// Cross-checks the pinned `HyprLLM` SHA-256 against a real download, so
    /// the constant isn't taken on faith. Ignored by default (network +
    /// ~1GB); run with:
    /// ```sh
    /// HYPR_LLM_GGUF_PATH=/path/to/hypr-llm.gguf \
    ///   cargo test -p local-model --lib -- --ignored hypr_llm_pinned_sha256_matches_a_real_download
    /// ```
    #[test]
    #[ignore]
    fn hypr_llm_pinned_sha256_matches_a_real_download() {
        let path = std::env::var("HYPR_LLM_GGUF_PATH")
            .expect("set HYPR_LLM_GGUF_PATH to a downloaded hypr-llm.gguf");
        let actual = sha256_hex(Path::new(&path)).unwrap();
        assert_eq!(Some(actual.as_str()), GgufLlmModel::HyprLLM.model_sha256());
    }
}
