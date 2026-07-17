pub use hypr_local_model::{
    AmModel, LocalModel, ParakeetOnnxModel, SoniqoModel, VoxtralLlamaModel, WhisperModel,
};

pub static SUPPORTED_MODELS: &[LocalModel] = &[
    LocalModel::Soniqo(SoniqoModel::ParakeetStreaming),
    LocalModel::Soniqo(SoniqoModel::ParakeetBatch),
    LocalModel::Am(AmModel::ParakeetV2),
    LocalModel::Am(AmModel::ParakeetV3),
    LocalModel::Am(AmModel::WhisperLargeV3),
    LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8),
    LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM),
    LocalModel::Whisper(WhisperModel::QuantizedTiny),
    LocalModel::Whisper(WhisperModel::QuantizedTinyEn),
    LocalModel::Whisper(WhisperModel::QuantizedBase),
    LocalModel::Whisper(WhisperModel::QuantizedBaseEn),
    LocalModel::Whisper(WhisperModel::QuantizedSmall),
    LocalModel::Whisper(WhisperModel::QuantizedSmallEn),
    LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo),
];

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttModelType {
    Soniqo,
    Whispercpp,
    Argmax,
    ParakeetOnnx,
    VoxtralLlama,
}

/// Language coverage summary for a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttModelLanguages {
    EnglishOnly,
    Multilingual,
}

/// Quality/speed trade-off tier, ordered fastest -> most accurate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttModelTier {
    Fastest,
    Fast,
    Balanced,
    Best,
}

/// What the model is the right pick for, given the live/final model split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SttRecommendedUse {
    Live,
    Final,
    LiveAndFinal,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SttModelInfo {
    pub key: LocalModel,
    pub display_name: String,
    pub description: String,
    pub size_bytes: Option<u64>,
    pub model_type: SttModelType,
    /// Runtime/model family, e.g. "whisper.cpp", "Parakeet", "Argmax".
    pub engine: String,
    pub languages: SttModelLanguages,
    /// Number of supported languages, when known. `None` for English-only
    /// models and for multilingual models without a pinned-down count.
    pub language_count: Option<u32>,
    pub tier: SttModelTier,
    pub recommended_use: SttRecommendedUse,
}

fn language_summary(
    english_only: bool,
    multilingual_count: Option<u32>,
) -> (SttModelLanguages, Option<u32>) {
    if english_only {
        (SttModelLanguages::EnglishOnly, None)
    } else {
        (SttModelLanguages::Multilingual, multilingual_count)
    }
}

fn whisper_traits(model: &WhisperModel) -> (SttModelTier, SttRecommendedUse) {
    match model {
        WhisperModel::QuantizedTiny | WhisperModel::QuantizedTinyEn => {
            (SttModelTier::Fastest, SttRecommendedUse::Live)
        }
        WhisperModel::QuantizedBase | WhisperModel::QuantizedBaseEn => {
            (SttModelTier::Fast, SttRecommendedUse::Live)
        }
        WhisperModel::QuantizedSmall | WhisperModel::QuantizedSmallEn => {
            (SttModelTier::Balanced, SttRecommendedUse::LiveAndFinal)
        }
        WhisperModel::QuantizedLargeTurbo => (SttModelTier::Best, SttRecommendedUse::Final),
    }
}

fn soniqo_traits(model: &SoniqoModel) -> (SttModelTier, SttRecommendedUse) {
    match model {
        SoniqoModel::ParakeetStreaming => (SttModelTier::Fastest, SttRecommendedUse::Live),
        SoniqoModel::ParakeetBatch | SoniqoModel::Omnilingual | SoniqoModel::Qwen3Small => {
            (SttModelTier::Balanced, SttRecommendedUse::Final)
        }
        SoniqoModel::Qwen3Large => (SttModelTier::Best, SttRecommendedUse::Final),
    }
}

fn parakeet_onnx_traits(_model: &ParakeetOnnxModel) -> (SttModelTier, SttRecommendedUse) {
    // TDT greedy decode on int8 CPU runs far faster than realtime while
    // matching Whisper-small-class accuracy: good live AND final default.
    (SttModelTier::Fast, SttRecommendedUse::LiveAndFinal)
}

fn voxtral_llama_traits(_model: &VoxtralLlamaModel) -> (SttModelTier, SttRecommendedUse) {
    // A 3B-parameter LLM decode loop (batch-only today, see
    // ggml-org/llama.cpp#20914) is far slower than a dedicated ASR model on
    // CPU; it earns its keep on quality/language coverage (incl. Hindi) on
    // CUDA hardware, so it's a final-pass pick, not a live one.
    (SttModelTier::Best, SttRecommendedUse::Final)
}

fn am_traits(model: &AmModel) -> (SttModelTier, SttRecommendedUse) {
    match model {
        AmModel::ParakeetV2 | AmModel::ParakeetV3 => {
            (SttModelTier::Balanced, SttRecommendedUse::LiveAndFinal)
        }
        AmModel::WhisperLargeV3 => (SttModelTier::Best, SttRecommendedUse::Final),
    }
}

pub fn stt_model_info(model: &LocalModel) -> SttModelInfo {
    match model {
        LocalModel::Soniqo(value) => {
            let (languages, language_count) =
                language_summary(value.is_english_only(), value.supported_language_count());
            let (tier, recommended_use) = soniqo_traits(value);
            SttModelInfo {
                key: model.clone(),
                display_name: value.display_name().to_string(),
                description: value.description().to_string(),
                size_bytes: Some(value.size_bytes()),
                model_type: SttModelType::Soniqo,
                engine: value.engine().to_string(),
                languages,
                language_count,
                tier,
                recommended_use,
            }
        }
        LocalModel::Whisper(value) => {
            let (languages, language_count) = language_summary(
                value.is_english_only(),
                Some(value.supported_languages().len() as u32),
            );
            let (tier, recommended_use) = whisper_traits(value);
            SttModelInfo {
                key: model.clone(),
                display_name: value.display_name().to_string(),
                description: value.description(),
                size_bytes: Some(value.model_size_bytes()),
                model_type: SttModelType::Whispercpp,
                engine: value.engine().to_string(),
                languages,
                language_count,
                tier,
                recommended_use,
            }
        }
        LocalModel::Am(value) => {
            let (languages, language_count) = language_summary(
                value.is_english_only(),
                Some(value.supported_languages().len() as u32),
            );
            let (tier, recommended_use) = am_traits(value);
            SttModelInfo {
                key: model.clone(),
                display_name: value.display_name().to_string(),
                description: value.description().to_string(),
                size_bytes: Some(value.model_size_bytes()),
                model_type: SttModelType::Argmax,
                engine: value.engine().to_string(),
                languages,
                language_count,
                tier,
                recommended_use,
            }
        }
        LocalModel::ParakeetOnnx(value) => {
            let (languages, language_count) = language_summary(
                value.is_english_only(),
                Some(value.supported_languages().len() as u32),
            );
            let (tier, recommended_use) = parakeet_onnx_traits(value);
            SttModelInfo {
                key: model.clone(),
                display_name: value.display_name().to_string(),
                description: value.description(),
                size_bytes: Some(value.model_size_bytes()),
                model_type: SttModelType::ParakeetOnnx,
                engine: value.engine().to_string(),
                languages,
                language_count,
                tier,
                recommended_use,
            }
        }
        LocalModel::VoxtralLlama(value) => {
            let (languages, language_count) = language_summary(
                value.is_english_only(),
                Some(value.supported_languages().len() as u32),
            );
            let (tier, recommended_use) = voxtral_llama_traits(value);
            SttModelInfo {
                key: model.clone(),
                display_name: value.display_name().to_string(),
                description: value.description(),
                size_bytes: Some(value.model_size_bytes()),
                model_type: SttModelType::VoxtralLlama,
                engine: value.engine().to_string(),
                languages,
                language_count,
                tier,
                recommended_use,
            }
        }
        LocalModel::GgufLlm(_) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_models_include_soniqo_models_from_rust_source_of_truth() {
        let supported_soniqo_models = SUPPORTED_MODELS
            .iter()
            .filter_map(|model| match model {
                LocalModel::Soniqo(value) => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(supported_soniqo_models, SoniqoModel::selectable());
    }

    #[test]
    fn soniqo_model_info_comes_from_soniqo_metadata() {
        for model in SoniqoModel::all() {
            let info = stt_model_info(&LocalModel::Soniqo(*model));

            assert_eq!(info.key, LocalModel::Soniqo(*model));
            assert_eq!(info.display_name, model.display_name());
            assert_eq!(info.description, model.description());
            assert_eq!(info.size_bytes, Some(model.size_bytes()));
            assert!(matches!(info.model_type, SttModelType::Soniqo));
            assert_eq!(info.engine, model.engine());
            assert_eq!(info.languages, SttModelLanguages::Multilingual);
        }
    }

    #[test]
    fn whisper_en_variants_are_english_only_and_others_multilingual() {
        for model in SUPPORTED_MODELS {
            let LocalModel::Whisper(whisper) = model else {
                continue;
            };

            let info = stt_model_info(model);
            if whisper.is_english_only() {
                assert_eq!(info.languages, SttModelLanguages::EnglishOnly);
                assert_eq!(info.language_count, None);
            } else {
                assert_eq!(info.languages, SttModelLanguages::Multilingual);
                // Whisper's multilingual checkpoints cover ~100 languages.
                assert!(info.language_count.unwrap_or(0) >= 50, "{whisper:?}");
            }
            assert_eq!(info.engine, "whisper.cpp");
        }
    }

    #[test]
    fn tier_and_recommended_use_pair_fast_models_with_live() {
        let tiny = stt_model_info(&LocalModel::Whisper(WhisperModel::QuantizedTiny));
        assert_eq!(tiny.tier, SttModelTier::Fastest);
        assert_eq!(tiny.recommended_use, SttRecommendedUse::Live);

        let small = stt_model_info(&LocalModel::Whisper(WhisperModel::QuantizedSmall));
        assert_eq!(small.tier, SttModelTier::Balanced);
        assert_eq!(small.recommended_use, SttRecommendedUse::LiveAndFinal);

        let turbo = stt_model_info(&LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo));
        assert_eq!(turbo.tier, SttModelTier::Best);
        assert_eq!(turbo.recommended_use, SttRecommendedUse::Final);

        let streaming = stt_model_info(&LocalModel::Soniqo(SoniqoModel::ParakeetStreaming));
        assert_eq!(streaming.tier, SttModelTier::Fastest);
        assert_eq!(streaming.recommended_use, SttRecommendedUse::Live);
    }

    #[test]
    fn every_supported_model_reports_an_engine_and_language_summary() {
        for model in SUPPORTED_MODELS {
            let info = stt_model_info(model);
            assert!(!info.engine.is_empty(), "{model:?} must name its engine");
            if info.languages == SttModelLanguages::EnglishOnly {
                assert_eq!(info.language_count, None, "{model:?}");
            }
        }
    }

    #[test]
    fn parakeet_onnx_model_is_supported_with_catalog_metadata() {
        let model = LocalModel::ParakeetOnnx(ParakeetOnnxModel::TdtV3Int8);
        assert!(SUPPORTED_MODELS.contains(&model));

        let info = stt_model_info(&model);
        assert_eq!(info.key, model);
        assert_eq!(info.display_name, "Parakeet TDT v3 (Multilingual)");
        assert_eq!(info.engine, "Parakeet (ONNX)");
        assert!(matches!(info.model_type, SttModelType::ParakeetOnnx));
        assert_eq!(info.languages, SttModelLanguages::Multilingual);
        assert_eq!(info.language_count, Some(25));
        assert_eq!(info.tier, SttModelTier::Fast);
        assert_eq!(info.recommended_use, SttRecommendedUse::LiveAndFinal);
        assert_eq!(info.size_bytes, Some(670_619_706));
    }

    #[test]
    fn parakeet_onnx_model_type_serializes_camel_case() {
        let json = serde_json::to_string(&SttModelType::ParakeetOnnx).unwrap();
        assert_eq!(json, "\"parakeetOnnx\"");
    }

    #[test]
    fn voxtral_llama_model_is_supported_with_catalog_metadata() {
        let model = LocalModel::VoxtralLlama(VoxtralLlamaModel::Mini3bQ4KM);
        assert!(SUPPORTED_MODELS.contains(&model));

        let info = stt_model_info(&model);
        assert_eq!(info.key, model);
        assert_eq!(info.display_name, "Voxtral Mini 3B (Multilingual)");
        assert_eq!(info.engine, "Voxtral (llama.cpp)");
        assert!(matches!(info.model_type, SttModelType::VoxtralLlama));
        assert_eq!(info.languages, SttModelLanguages::Multilingual);
        assert_eq!(info.language_count, Some(8));
        assert_eq!(info.tier, SttModelTier::Best);
        assert_eq!(info.recommended_use, SttRecommendedUse::Final);
        assert_eq!(info.size_bytes, Some(3_188_716_000));
    }

    #[test]
    fn voxtral_llama_model_type_serializes_camel_case() {
        let json = serde_json::to_string(&SttModelType::VoxtralLlama).unwrap();
        assert_eq!(json, "\"voxtralLlama\"");
    }

    #[test]
    fn am_models_report_argmax_engine_and_language_scope() {
        let v2 = stt_model_info(&LocalModel::Am(AmModel::ParakeetV2));
        assert_eq!(v2.engine, "Argmax");
        assert_eq!(v2.languages, SttModelLanguages::EnglishOnly);

        let v3 = stt_model_info(&LocalModel::Am(AmModel::ParakeetV3));
        assert_eq!(v3.languages, SttModelLanguages::Multilingual);
        assert_eq!(v3.language_count, Some(25));
    }
}
