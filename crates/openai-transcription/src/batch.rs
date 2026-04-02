use std::fmt;

use serde::{Deserialize, Serialize};

pub const MODEL_WHISPER_1: &str = "whisper-1";
pub const MODEL_GPT_4O_TRANSCRIBE: &str = "gpt-4o-transcribe";
pub const MODEL_GPT_4O_MINI_TRANSCRIBE: &str = "gpt-4o-mini-transcribe";
pub const MODEL_GPT_4O_MINI_TRANSCRIBE_2025_12_15: &str = "gpt-4o-mini-transcribe-2025-12-15";
pub const MODEL_GPT_4O_TRANSCRIBE_DIARIZE: &str = "gpt-4o-transcribe-diarize";

pub fn supports_timestamp_granularities(model: &str) -> bool {
    model == MODEL_WHISPER_1
}

pub fn default_response_format(model: &str) -> AudioResponseFormat {
    if supports_timestamp_granularities(model) {
        AudioResponseFormat::VerboseJson
    } else {
        AudioResponseFormat::Json
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateTranscriptionOptions {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<TranscriptionInclude>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_speaker_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_speaker_references: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AudioResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timestamp_granularities: Vec<TimestampGranularity>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerVadType {
    #[serde(rename = "server_vad")]
    ServerVad,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscriptionInclude {
    #[serde(rename = "logprobs")]
    Logprobs,
}

impl fmt::Display for TranscriptionInclude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Logprobs => "logprobs",
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioResponseFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "srt")]
    Srt,
    #[serde(rename = "verbose_json")]
    VerboseJson,
    #[serde(rename = "vtt")]
    Vtt,
    #[serde(rename = "diarized_json")]
    DiarizedJson,
}

impl fmt::Display for AudioResponseFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Json => "json",
            Self::Text => "text",
            Self::Srt => "srt",
            Self::VerboseJson => "verbose_json",
            Self::Vtt => "vtt",
            Self::DiarizedJson => "diarized_json",
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimestampGranularity {
    #[serde(rename = "word")]
    Word,
    #[serde(rename = "segment")]
    Segment,
}

impl fmt::Display for TimestampGranularity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Word => "word",
            Self::Segment => "segment",
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CreateTranscriptionResponse {
    Diarized(DiarizedTranscriptionResponse),
    Standard(TranscriptionResponse),
}

impl CreateTranscriptionResponse {
    pub fn text(&self) -> &str {
        match self {
            Self::Diarized(response) => &response.text,
            Self::Standard(response) => &response.text,
        }
    }

    pub fn language(&self) -> Option<&str> {
        match self {
            Self::Diarized(_) => None,
            Self::Standard(response) => response.language.as_deref(),
        }
    }

    pub fn words(&self) -> &[TranscriptionWord] {
        match self {
            Self::Diarized(_) => &[],
            Self::Standard(response) => response.words.as_slice(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionResponse {
    pub task: Option<String>,
    pub language: Option<String>,
    pub duration: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub words: Vec<TranscriptionWord>,
    #[serde(default)]
    pub segments: Vec<TranscriptionSegment>,
    #[serde(default)]
    pub logprobs: Vec<TranscriptionLogprob>,
    pub usage: Option<TranscriptionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionSegment {
    pub id: Option<u64>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub text: Option<String>,
    #[serde(rename = "type")]
    pub segment_type: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionLogprob {
    pub token: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub logprob: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TranscriptionUsage {
    Tokens(TokenUsage),
    Duration(DurationUsage),
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(rename = "type")]
    pub usage_type: String,
    pub input_token_details: Option<InputTokenDetails>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputTokenDetails {
    pub audio_tokens: Option<u64>,
    pub text_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DurationUsage {
    pub seconds: f64,
    #[serde(rename = "type")]
    pub usage_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiarizedTranscriptionResponse {
    pub duration: f64,
    pub segments: Vec<DiarizedTranscriptionSegment>,
    pub task: String,
    pub text: String,
    pub usage: Option<TranscriptionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiarizedTranscriptionSegment {
    pub id: String,
    pub end: f64,
    pub speaker: String,
    pub start: f64,
    pub text: String,
    #[serde(rename = "type")]
    pub segment_type: String,
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
    fn default_response_format_matches_model_capabilities() {
        assert_eq!(
            default_response_format(MODEL_WHISPER_1),
            AudioResponseFormat::VerboseJson
        );
        assert_eq!(
            default_response_format(MODEL_GPT_4O_TRANSCRIBE),
            AudioResponseFormat::Json
        );
    }

    #[test]
    fn parses_standard_response_with_words() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "text": "hello world",
                "language": "en",
                "words": [
                    { "word": "hello", "start": 0.0, "end": 0.5 },
                    { "word": "world", "start": 0.5, "end": 1.0 }
                ]
            }"#,
        )
        .expect("parse response");

        assert_eq!(response.text(), "hello world");
        assert_eq!(response.language(), Some("en"));
        assert_eq!(response.words().len(), 2);
    }
}
