use std::path::{Path, PathBuf};

pub use hypr_am::AmModel;
use hypr_model_downloader::{DownloadPart, DownloadableModel, Error};
pub use hypr_parakeet_onnx_model::ParakeetOnnxModel;
pub use hypr_transcribe_soniqo::SoniqoModel;
pub use hypr_whisper_local_model::WhisperModel;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq)]
pub enum GgufLlmModel {
    Llama3p2_3bQ4,
    Gemma3_4bQ4,
    HyprLLM,
}

impl GgufLlmModel {
    pub fn file_name(&self) -> &str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "llm.gguf",
            GgufLlmModel::HyprLLM => "hypr-llm.gguf",
            GgufLlmModel::Gemma3_4bQ4 => "gemma-3-4b-it-Q4_K_M.gguf",
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
        }
    }

    pub fn model_size(&self) -> u64 {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => 2019377440,
            GgufLlmModel::HyprLLM => 1107409056,
            GgufLlmModel::Gemma3_4bQ4 => 2489894016,
        }
    }

    pub fn model_checksum(&self) -> u32 {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => 2831308098,
            GgufLlmModel::HyprLLM => 4037351144,
            GgufLlmModel::Gemma3_4bQ4 => 2760830291,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "Llama 3.2 3B Q4",
            GgufLlmModel::HyprLLM => "HyprLLM",
            GgufLlmModel::Gemma3_4bQ4 => "Gemma 3 4B Q4",
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
        ]);

        models.extend([
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4),
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM),
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4),
        ]);

        models
    }

    pub fn kind(&self) -> &'static str {
        match self {
            LocalModel::Soniqo(_) => "stt-soniqo",
            LocalModel::Whisper(_) => "stt-whisper",
            LocalModel::Am(_) => "stt-am",
            LocalModel::ParakeetOnnx(_) => "stt-parakeet-onnx",
            LocalModel::GgufLlm(_) => "llm",
        }
    }

    pub fn model_kind(&self) -> LocalModelKind {
        match self {
            LocalModel::Soniqo(_)
            | LocalModel::Whisper(_)
            | LocalModel::Am(_)
            | LocalModel::ParakeetOnnx(_) => LocalModelKind::Stt,
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
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4) => "llm-llama3-2-3b-q4",
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM) => "llm-hypr-llm",
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4) => "llm-gemma3-4b-q4",
        }
    }

    pub fn install_path(&self, models_base: &Path) -> PathBuf {
        match self {
            LocalModel::Soniqo(model) => models_base.join("soniqo").join(model.as_str()),
            LocalModel::Whisper(model) => models_base.join("stt").join(model.file_name()),
            LocalModel::Am(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::ParakeetOnnx(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::GgufLlm(model) => models_base.join("llm").join(model.file_name()),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            LocalModel::Soniqo(model) => model.display_name().to_string(),
            LocalModel::Whisper(model) => model.display_name().to_string(),
            LocalModel::Am(model) => model.display_name().to_string(),
            LocalModel::ParakeetOnnx(model) => model.display_name().to_string(),
            LocalModel::GgufLlm(model) => model.display_name().to_string(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            LocalModel::Soniqo(model) => model.description().to_string(),
            LocalModel::Whisper(model) => model.description(),
            LocalModel::Am(model) => model.description().to_string(),
            LocalModel::ParakeetOnnx(model) => model.description(),
            LocalModel::GgufLlm(model) => model.description(),
        }
    }

    pub fn is_available_on_current_platform(&self) -> bool {
        let is_apple_silicon = cfg!(target_arch = "aarch64") && cfg!(target_os = "macos");

        match self {
            LocalModel::Soniqo(model) => model.is_available_on_current_platform(),
            // whisper.cpp runs on every desktop platform (CPU at minimum);
            // GPU offload (Metal/Vulkan/CUDA) is a build-feature concern.
            LocalModel::Whisper(_) => true,
            LocalModel::Am(_) => is_apple_silicon,
            // ONNX Runtime CPU execution works on every desktop platform.
            LocalModel::ParakeetOnnx(_) => true,
            LocalModel::GgufLlm(_) => cfg!(target_arch = "aarch64"),
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
        Some(self.model_checksum())
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

    fn finalize_download(&self, _downloaded_path: &Path, _models_base: &Path) -> Result<(), Error> {
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
    fn soniqo_models_reject_generic_download_finalize() {
        let model = LocalModel::Soniqo(SoniqoModel::ParakeetStreaming);

        let error = model
            .finalize_download(Path::new("download"), Path::new("models"))
            .unwrap_err();

        assert!(error.to_string().contains("Soniqo bridge"));
    }
}
