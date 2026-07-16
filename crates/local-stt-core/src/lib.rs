pub use hypr_local_model::{AmModel, LocalModel, SoniqoModel, WhisperModel};

pub static SUPPORTED_MODELS: &[LocalModel] = &[
    LocalModel::Soniqo(SoniqoModel::ParakeetStreaming),
    LocalModel::Soniqo(SoniqoModel::ParakeetBatch),
    LocalModel::Am(AmModel::ParakeetV2),
    LocalModel::Am(AmModel::ParakeetV3),
    LocalModel::Am(AmModel::WhisperLargeV3),
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
    fn am_models_report_argmax_engine_and_language_scope() {
        let v2 = stt_model_info(&LocalModel::Am(AmModel::ParakeetV2));
        assert_eq!(v2.engine, "Argmax");
        assert_eq!(v2.languages, SttModelLanguages::EnglishOnly);

        let v3 = stt_model_info(&LocalModel::Am(AmModel::ParakeetV3));
        assert_eq!(v3.languages, SttModelLanguages::Multilingual);
        assert_eq!(v3.language_count, Some(25));
    }
}
