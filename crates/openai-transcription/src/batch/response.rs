use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CreateTranscriptionResponse {
    Diarized(TranscriptionDiarizedResponse),
    Verbose(TranscriptionVerboseResponse),
    Standard(TranscriptionResponse),
}

impl CreateTranscriptionResponse {
    pub fn text(&self) -> &str {
        match self {
            Self::Diarized(response) => &response.text,
            Self::Verbose(response) => &response.text,
            Self::Standard(response) => &response.text,
        }
    }

    pub fn language(&self) -> Option<&str> {
        match self {
            Self::Diarized(_) | Self::Standard(_) => None,
            Self::Verbose(response) => Some(response.language.as_str()),
        }
    }

    pub fn words(&self) -> &[TranscriptionWord] {
        match self {
            Self::Diarized(_) | Self::Standard(_) => &[],
            Self::Verbose(response) => response.words.as_slice(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionResponse {
    pub text: String,
    #[serde(default)]
    pub logprobs: Vec<TranscriptionLogprob>,
    pub usage: Option<TranscriptionUsage>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionVerboseResponse {
    pub duration: f64,
    pub language: String,
    pub text: String,
    #[serde(default)]
    pub segments: Vec<TranscriptionSegment>,
    pub usage: Option<DurationUsage>,
    #[serde(default)]
    pub words: Vec<TranscriptionWord>,
    #[serde(default)]
    pub task: Option<TranscriptionTask>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    pub id: u64,
    pub avg_logprob: f64,
    pub compression_ratio: f64,
    pub end: f64,
    pub no_speech_prob: f64,
    pub seek: u64,
    pub start: f64,
    pub temperature: f32,
    pub text: String,
    pub tokens: Vec<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionLogprob {
    pub token: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub logprob: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TranscriptionUsage {
    Tokens(TokenUsage),
    Duration(DurationUsage),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(rename = "type")]
    pub usage_type: TokenUsageType,
    pub input_token_details: Option<InputTokenDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputTokenDetails {
    pub audio_tokens: Option<u64>,
    pub text_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DurationUsage {
    pub seconds: f64,
    #[serde(rename = "type")]
    pub usage_type: DurationUsageType,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TokenUsageType {
    #[serde(rename = "tokens")]
    #[strum(serialize = "tokens")]
    Tokens,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum DurationUsageType {
    #[serde(rename = "duration")]
    #[strum(serialize = "duration")]
    Duration,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TranscriptionTask {
    #[serde(rename = "transcribe")]
    #[strum(serialize = "transcribe")]
    Transcribe,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionDiarizedResponse {
    pub duration: f64,
    pub segments: Vec<TranscriptionDiarizedSegment>,
    pub task: TranscriptionTask,
    pub text: String,
    pub usage: Option<TranscriptionUsage>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TranscriptionDiarizedSegment {
    pub id: String,
    pub end: f64,
    pub speaker: String,
    pub start: f64,
    pub text: String,
    #[serde(rename = "type")]
    pub segment_type: TranscriptionDiarizedSegmentType,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumString, Display, AsRefStr,
)]
pub enum TranscriptionDiarizedSegmentType {
    #[serde(rename = "transcript.text.segment")]
    #[strum(serialize = "transcript.text.segment")]
    TranscriptTextSegment,
}

pub type DiarizedTranscriptionResponse = TranscriptionDiarizedResponse;
pub type DiarizedTranscriptionSegment = TranscriptionDiarizedSegment;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TranscriptionStreamEvent {
    #[serde(rename = "transcript.text.delta")]
    TextDelta {
        delta: String,
        #[serde(default)]
        logprobs: Vec<TranscriptionLogprob>,
    },
    #[serde(rename = "transcript.text.done")]
    TextDone {
        text: String,
        #[serde(default)]
        logprobs: Vec<TranscriptionLogprob>,
        usage: Option<TranscriptionUsage>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_response() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "text": "hello world",
                "logprobs": [
                    { "token": "hello", "logprob": -0.1, "bytes": [104, 101, 108, 108, 111] }
                ],
                "usage": {
                    "type": "tokens",
                    "input_tokens": 1,
                    "output_tokens": 2,
                    "total_tokens": 3
                }
            }"#,
        )
        .expect("parse response");

        assert_eq!(response.text(), "hello world");
        assert_eq!(response.language(), None);
        assert!(response.words().is_empty());
    }

    #[test]
    fn parses_verbose_response_with_words() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "task": "transcribe",
                "duration": 8.47,
                "language": "english",
                "text": "hello world",
                "words": [
                    { "word": "hello", "start": 0.0, "end": 0.5 },
                    { "word": "world", "start": 0.5, "end": 1.0 }
                ],
                "usage": {
                    "type": "duration",
                    "seconds": 9
                }
            }"#,
        )
        .expect("parse verbose response");

        assert_eq!(response.text(), "hello world");
        assert_eq!(response.language(), Some("english"));
        assert_eq!(response.words().len(), 2);
    }

    #[test]
    fn parses_diarized_response() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "task": "transcribe",
                "duration": 27.4,
                "text": "Agent: hi",
                "segments": [
                    {
                        "type": "transcript.text.segment",
                        "id": "seg_001",
                        "start": 0.0,
                        "end": 4.7,
                        "text": "hi",
                        "speaker": "agent"
                    }
                ],
                "usage": {
                    "type": "duration",
                    "seconds": 27
                }
            }"#,
        )
        .expect("parse diarized response");

        assert_eq!(response.text(), "Agent: hi");
        assert!(response.words().is_empty());
    }

    #[test]
    fn parses_stream_events() {
        let delta: TranscriptionStreamEvent = serde_json::from_str(
            r#"{
                "type": "transcript.text.delta",
                "delta": "I",
                "logprobs": [{ "token": "I", "logprob": -0.1, "bytes": [73] }]
            }"#,
        )
        .expect("parse delta");
        let done: TranscriptionStreamEvent = serde_json::from_str(
            r#"{
                "type": "transcript.text.done",
                "text": "I see skies of blue.",
                "logprobs": [{ "token": ".", "logprob": -0.1, "bytes": [46] }],
                "usage": {
                    "type": "tokens",
                    "input_tokens": 14,
                    "output_tokens": 45,
                    "total_tokens": 59
                }
            }"#,
        )
        .expect("parse done");

        assert!(matches!(delta, TranscriptionStreamEvent::TextDelta { .. }));
        assert!(matches!(done, TranscriptionStreamEvent::TextDone { .. }));
    }
}
