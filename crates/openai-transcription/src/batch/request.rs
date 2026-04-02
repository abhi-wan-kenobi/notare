use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

use super::model::{AudioModel, GptTranscriptionModel};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommonTranscriptionOptions {
    pub chunking_strategy: Option<ChunkingStrategy>,
    pub known_speaker_names: Vec<String>,
    pub known_speaker_references: Vec<String>,
    pub language: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateWhisperTranscriptionOptions {
    pub common: CommonTranscriptionOptions,
    pub prompt: Option<String>,
    pub response_format: Option<WhisperResponseFormat>,
    pub timestamp_granularities: Vec<TimestampGranularity>,
}

impl Default for CreateWhisperTranscriptionOptions {
    fn default() -> Self {
        Self {
            common: CommonTranscriptionOptions::default(),
            prompt: None,
            response_format: None,
            timestamp_granularities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateGptTranscriptionOptions {
    pub model: GptTranscriptionModel,
    pub common: CommonTranscriptionOptions,
    pub include: Vec<TranscriptionInclude>,
    pub prompt: Option<String>,
    pub response_format: Option<GptResponseFormat>,
    pub stream: Option<bool>,
}

impl Default for CreateGptTranscriptionOptions {
    fn default() -> Self {
        Self {
            model: GptTranscriptionModel::Gpt4oTranscribe,
            common: CommonTranscriptionOptions::default(),
            include: Vec::new(),
            prompt: None,
            response_format: None,
            stream: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateDiarizedTranscriptionOptions {
    pub common: CommonTranscriptionOptions,
    pub response_format: Option<DiarizedResponseFormat>,
    pub stream: Option<bool>,
}

impl Default for CreateDiarizedTranscriptionOptions {
    fn default() -> Self {
        Self {
            common: CommonTranscriptionOptions::default(),
            response_format: None,
            stream: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CreateTranscriptionOptions {
    Whisper(CreateWhisperTranscriptionOptions),
    Gpt(CreateGptTranscriptionOptions),
    Diarize(CreateDiarizedTranscriptionOptions),
}

impl CreateTranscriptionOptions {
    pub fn whisper() -> Self {
        Self::Whisper(CreateWhisperTranscriptionOptions::default())
    }

    pub fn gpt(model: GptTranscriptionModel) -> Self {
        Self::Gpt(CreateGptTranscriptionOptions {
            model,
            ..Default::default()
        })
    }

    pub fn diarize() -> Self {
        Self::Diarize(CreateDiarizedTranscriptionOptions::default())
    }

    pub fn model(&self) -> AudioModel {
        match self {
            Self::Whisper(_) => AudioModel::Whisper1,
            Self::Gpt(options) => options.model.into(),
            Self::Diarize(_) => AudioModel::Gpt4oTranscribeDiarize,
        }
    }

    pub fn common(&self) -> &CommonTranscriptionOptions {
        match self {
            Self::Whisper(options) => &options.common,
            Self::Gpt(options) => &options.common,
            Self::Diarize(options) => &options.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut CommonTranscriptionOptions {
        match self {
            Self::Whisper(options) => &mut options.common,
            Self::Gpt(options) => &mut options.common,
            Self::Diarize(options) => &mut options.common,
        }
    }
}

impl Default for CreateTranscriptionOptions {
    fn default() -> Self {
        Self::gpt(GptTranscriptionModel::Gpt4oTranscribe)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ChunkingStrategy {
    Auto(AutoChunkingStrategy),
    ServerVad(ServerVadConfig),
}

impl ChunkingStrategy {
    pub fn auto() -> Self {
        Self::Auto(AutoChunkingStrategy::Auto)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutoChunkingStrategy {
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerVadConfig {
    #[serde(rename = "type")]
    pub kind: ServerVadType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum ServerVadType {
    #[serde(rename = "server_vad")]
    #[strum(serialize = "server_vad")]
    ServerVad,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TranscriptionInclude {
    #[serde(rename = "logprobs")]
    #[strum(serialize = "logprobs")]
    Logprobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum WhisperResponseFormat {
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "text")]
    Text,
    #[strum(serialize = "srt")]
    Srt,
    #[strum(serialize = "verbose_json")]
    VerboseJson,
    #[strum(serialize = "vtt")]
    Vtt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum GptResponseFormat {
    #[strum(serialize = "json")]
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr)]
pub enum DiarizedResponseFormat {
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "text")]
    Text,
    #[strum(serialize = "diarized_json")]
    DiarizedJson,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum AudioResponseFormat {
    #[serde(rename = "json")]
    #[strum(serialize = "json")]
    Json,
    #[serde(rename = "text")]
    #[strum(serialize = "text")]
    Text,
    #[serde(rename = "srt")]
    #[strum(serialize = "srt")]
    Srt,
    #[serde(rename = "verbose_json")]
    #[strum(serialize = "verbose_json")]
    VerboseJson,
    #[serde(rename = "vtt")]
    #[strum(serialize = "vtt")]
    Vtt,
    #[serde(rename = "diarized_json")]
    #[strum(serialize = "diarized_json")]
    DiarizedJson,
}

impl From<WhisperResponseFormat> for AudioResponseFormat {
    fn from(value: WhisperResponseFormat) -> Self {
        match value {
            WhisperResponseFormat::Json => Self::Json,
            WhisperResponseFormat::Text => Self::Text,
            WhisperResponseFormat::Srt => Self::Srt,
            WhisperResponseFormat::VerboseJson => Self::VerboseJson,
            WhisperResponseFormat::Vtt => Self::Vtt,
        }
    }
}

impl From<GptResponseFormat> for AudioResponseFormat {
    fn from(_: GptResponseFormat) -> Self {
        Self::Json
    }
}

impl From<DiarizedResponseFormat> for AudioResponseFormat {
    fn from(value: DiarizedResponseFormat) -> Self {
        match value {
            DiarizedResponseFormat::Json => Self::Json,
            DiarizedResponseFormat::Text => Self::Text,
            DiarizedResponseFormat::DiarizedJson => Self::DiarizedJson,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TimestampGranularity {
    #[serde(rename = "word")]
    #[strum(serialize = "word")]
    Word,
    #[serde(rename = "segment")]
    #[strum(serialize = "segment")]
    Segment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_strategy_auto_serializes_as_string() {
        let json = serde_json::to_string(&ChunkingStrategy::auto()).expect("serialize auto");

        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn request_variants_preserve_model_identity() {
        assert_eq!(
            CreateTranscriptionOptions::whisper().model(),
            AudioModel::Whisper1
        );
        assert_eq!(
            CreateTranscriptionOptions::gpt(GptTranscriptionModel::Gpt4oMiniTranscribe).model(),
            AudioModel::Gpt4oMiniTranscribe
        );
        assert_eq!(
            CreateTranscriptionOptions::diarize().model(),
            AudioModel::Gpt4oTranscribeDiarize
        );
    }
}
