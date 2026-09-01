use std::path::{Path, PathBuf};

pub use hypr_local_model::GgufLlmModel as SupportedModel;

/// The models this build can actually download and run. Previously this was
/// hardcoded to all three models under `#[cfg(target_arch = "aarch64")]` and
/// **empty on every other architecture** — even though `HyprLLM`, the one
/// non-deprecated model and the one the embedded local LLM server ships,
/// runs anywhere `llama-cpp-2` builds (every desktop target this workspace
/// targets; GPU offload is a separate `cuda`/`vulkan` build-feature
/// concern). `Llama3p2_3bQ4` and `Gemma3_4bQ4` are self-described as
/// "Deprecated. Exists only for backward compatibility." (see
/// `supported_model_info`) and keep their original aarch64-only gate rather
/// than being newly widened — this mirrors
/// `GgufLlmModel::is_available_on_current_platform()`, which is the same
/// per-model decision `LocalModel::is_available_on_current_platform` uses
/// elsewhere in the workspace.
#[cfg(target_arch = "aarch64")]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[
    SupportedModel::HyprLLM,
    SupportedModel::Llama3p2_3bQ4,
    SupportedModel::Gemma3_4bQ4,
];

#[cfg(not(target_arch = "aarch64"))]
pub static SUPPORTED_MODELS: &[SupportedModel] = &[SupportedModel::HyprLLM];

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ModelInfo {
    pub key: SupportedModel,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CustomModelInfo {
    pub path: String,
    pub name: String,
}

pub fn llm_models_dir(models_base: &Path) -> PathBuf {
    models_base.join("llm")
}

/// Previously this unconditionally listed all three models regardless of
/// `SUPPORTED_MODELS` — so a non-aarch64 build advertised two models
/// `is_downloaded`/`reconcile` would never recognize (their file names don't
/// match anything in an empty `SUPPORTED_MODELS`). Deriving from
/// `SUPPORTED_MODELS` makes "listed" and "actually supported" the same set.
pub fn list_supported_models() -> Vec<ModelInfo> {
    SUPPORTED_MODELS.iter().map(supported_model_info).collect()
}

pub fn supported_model_info(model: &SupportedModel) -> ModelInfo {
    let description = match model {
        SupportedModel::HyprLLM => "Experimental model trained by the Char team.",
        SupportedModel::Gemma3_4bQ4 | SupportedModel::Llama3p2_3bQ4 => {
            "Deprecated. Exists only for backward compatibility."
        }
    };

    ModelInfo {
        key: model.clone(),
        name: model.display_name().to_string(),
        description: description.to_string(),
        size_bytes: model.model_size(),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ModelIdentifier {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "mock-onboarding")]
    MockOnboarding,
}
