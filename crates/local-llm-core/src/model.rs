use std::path::{Path, PathBuf};

pub use hypr_local_model::GgufLlmModel as SupportedModel;

/// The models this build can download and run.
pub static SUPPORTED_MODELS: &[SupportedModel] = &[
    SupportedModel::Qwen3_4bQ4,
    SupportedModel::Gemma3_4bQ4,
    SupportedModel::Phi4MiniQ4,
    SupportedModel::Llama3p1_8bQ4,
    SupportedModel::Mistral7bV03Q4,
];

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
    SUPPORTED_MODELS
        .iter()
        .filter(|m| m.is_available_on_current_platform())
        .map(supported_model_info)
        .collect()
}

pub fn supported_model_info(model: &SupportedModel) -> ModelInfo {
    let description = match model {
        SupportedModel::Qwen3_4bQ4 => {
            "Recommended for most devices. Strong multilingual summaries and chat."
        }
        SupportedModel::Gemma3_4bQ4 => "Fast general-purpose model from Google.",
        SupportedModel::Phi4MiniQ4 => "Compact Microsoft model with strong reasoning.",
        SupportedModel::Llama3p1_8bQ4 => "Best general quality. Suited to higher-memory devices.",
        SupportedModel::Mistral7bV03Q4 => "Reliable summaries and instruction following.",
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
