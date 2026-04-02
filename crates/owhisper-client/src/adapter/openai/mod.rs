mod batch;
mod live;

use openai_transcription::batch::AudioModel;

use crate::providers::Provider;

use super::{LanguageQuality, LanguageSupport};

#[derive(Clone, Default)]
pub struct OpenAIAdapter;

impl OpenAIAdapter {
    pub fn language_support_live(_languages: &[hypr_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }

    pub fn language_support_batch(_languages: &[hypr_language::Language]) -> LanguageSupport {
        Self::language_support_live(_languages)
    }

    pub fn is_supported_languages_live(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    pub fn is_supported_languages_batch(languages: &[hypr_language::Language]) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    pub(crate) fn resolve_batch_model(model: Option<&str>) -> AudioModel {
        let default = Provider::OpenAI.default_batch_model();

        match model {
            Some(value) if crate::providers::is_meta_model(value) => {
                default.parse().expect("invalid_default_openai_batch_model")
            }
            Some(value) => value
                .parse()
                .unwrap_or_else(|_| default.parse().expect("invalid_default_openai_batch_model")),
            None => default.parse().expect("invalid_default_openai_batch_model"),
        }
    }

    pub fn supports_progressive_batch_model(model: Option<&str>) -> bool {
        matches!(
            Self::resolve_batch_model(model),
            AudioModel::Gpt4oTranscribe
                | AudioModel::Gpt4oMiniTranscribe
                | AudioModel::Gpt4oMiniTranscribe20251215
        )
    }

    pub(crate) fn build_ws_url_from_base(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        if api_base.is_empty() {
            return (
                Provider::OpenAI
                    .default_ws_url()
                    .parse()
                    .expect("invalid_default_ws_url"),
                vec![("intent".to_string(), "transcription".to_string())],
            );
        }

        if let Some(proxy_result) = super::build_proxy_ws_url(api_base) {
            return proxy_result;
        }

        let parsed: url::Url = api_base.parse().expect("invalid_api_base");
        let mut existing_params = super::extract_query_params(&parsed);

        if !existing_params.iter().any(|(k, _)| k == "intent") {
            existing_params.push(("intent".to_string(), "transcription".to_string()));
        }

        let host = parsed
            .host_str()
            .unwrap_or(Provider::OpenAI.default_ws_host());
        let mut url: url::Url = format!("wss://{}{}", host, Provider::OpenAI.ws_path())
            .parse()
            .expect("invalid_ws_url");

        super::set_scheme_from_host(&mut url);

        (url, existing_params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_from_base_empty() {
        let (url, params) = OpenAIAdapter::build_ws_url_from_base("");
        assert_eq!(url.as_str(), "wss://api.openai.com/v1/realtime");
        assert_eq!(
            params,
            vec![("intent".to_string(), "transcription".to_string())]
        );
    }

    #[test]
    fn test_build_ws_url_from_base_proxy() {
        let (url, params) =
            OpenAIAdapter::build_ws_url_from_base("https://api.hyprnote.com?provider=openai");
        assert_eq!(url.as_str(), "wss://api.hyprnote.com/listen");
        assert_eq!(params, vec![("provider".to_string(), "openai".to_string())]);
    }

    #[test]
    fn test_build_ws_url_from_base_localhost() {
        let (url, params) =
            OpenAIAdapter::build_ws_url_from_base("http://localhost:8787?provider=openai");
        assert_eq!(url.as_str(), "ws://localhost:8787/listen");
        assert_eq!(params, vec![("provider".to_string(), "openai".to_string())]);
    }

    #[test]
    fn test_is_openai_host() {
        assert!(Provider::OpenAI.is_host("api.openai.com"));
        assert!(Provider::OpenAI.is_host("openai.com"));
        assert!(!Provider::OpenAI.is_host("api.deepgram.com"));
    }

    #[test]
    fn resolve_batch_model_defaults_to_diarize() {
        assert_eq!(
            OpenAIAdapter::resolve_batch_model(None),
            AudioModel::Gpt4oTranscribeDiarize
        );
    }

    #[test]
    fn progressive_batch_only_supports_non_diarized_gpt_models() {
        assert!(OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-transcribe"
        )));
        assert!(OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-mini-transcribe"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(Some(
            "gpt-4o-transcribe-diarize"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(Some(
            "whisper-1"
        )));
        assert!(!OpenAIAdapter::supports_progressive_batch_model(None));
    }
}
