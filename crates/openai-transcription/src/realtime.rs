use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct SessionUpdateEvent {
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionConfig {
    #[serde(rename = "type")]
    pub session_type: SessionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<SessionInclude>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<AudioInputConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioInputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<AudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription: Option<TranscriptionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<TurnDetectionConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioFormat {
    #[serde(rename = "type")]
    pub format_type: AudioFormatType,
    pub rate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionConfig {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnDetectionConfig {
    #[serde(rename = "type")]
    pub detection_type: TurnDetectionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputAudioBufferAppendEvent {
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
    pub audio: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputAudioBufferCommitEvent {
    #[serde(rename = "type")]
    pub event_type: ClientEventType,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ClientEventType {
    #[serde(rename = "session.update")]
    SessionUpdate,
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend,
    #[serde(rename = "input_audio_buffer.commit")]
    InputAudioBufferCommit,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SessionType {
    #[serde(rename = "transcription")]
    Transcription,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum AudioFormatType {
    #[serde(rename = "audio/pcm")]
    AudioPcm,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TurnDetectionType {
    #[serde(rename = "server_vad")]
    ServerVad,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SessionInclude {
    #[serde(rename = "item.input_audio_transcription.logprobs")]
    InputAudioTranscriptionLogprobs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    #[serde(rename = "session.created")]
    SessionCreated { session: SessionInfo },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: SessionInfo },
    #[serde(rename = "input_audio_buffer.committed")]
    InputAudioBufferCommitted { item_id: String },
    #[serde(rename = "input_audio_buffer.cleared")]
    InputAudioBufferCleared,
    #[serde(rename = "input_audio_buffer.speech_started")]
    InputAudioBufferSpeechStarted { item_id: String },
    #[serde(rename = "input_audio_buffer.speech_stopped")]
    InputAudioBufferSpeechStopped { item_id: String },
    #[serde(rename = "conversation.item.input_audio_transcription.completed")]
    ConversationItemInputAudioTranscriptionCompleted {
        item_id: String,
        content_index: u32,
        transcript: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.delta")]
    ConversationItemInputAudioTranscriptionDelta {
        item_id: String,
        content_index: u32,
        delta: String,
    },
    #[serde(rename = "conversation.item.input_audio_transcription.failed")]
    ConversationItemInputAudioTranscriptionFailed {
        item_id: String,
        content_index: u32,
        error: ErrorInfo,
    },
    #[serde(rename = "error")]
    Error { error: ErrorInfo },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_update_serializes_expected_shape() {
        let json = serde_json::to_value(SessionUpdateEvent {
            event_type: ClientEventType::SessionUpdate,
            session: SessionConfig {
                session_type: SessionType::Transcription,
                audio: Some(AudioConfig {
                    input: Some(AudioInputConfig {
                        format: Some(AudioFormat {
                            format_type: AudioFormatType::AudioPcm,
                            rate: 24_000,
                        }),
                        transcription: Some(TranscriptionConfig {
                            model: "gpt-4o-transcribe".to_string(),
                            language: Some("en".to_string()),
                        }),
                        turn_detection: Some(TurnDetectionConfig {
                            detection_type: TurnDetectionType::ServerVad,
                            threshold: Some(0.5),
                            prefix_padding_ms: Some(300),
                            silence_duration_ms: Some(500),
                        }),
                    }),
                }),
                include: Some(vec![SessionInclude::InputAudioTranscriptionLogprobs]),
            },
        })
        .expect("serialize session");

        assert_eq!(json["type"], "session.update");
        assert_eq!(json["session"]["type"], "transcription");
        assert_eq!(
            json["session"]["audio"]["input"]["format"]["type"],
            "audio/pcm"
        );
    }

    #[test]
    fn parses_completed_server_event() {
        let event: ServerEvent = serde_json::from_str(
            r#"{
                "type": "conversation.item.input_audio_transcription.completed",
                "item_id": "item-123",
                "content_index": 0,
                "transcript": "hello world"
            }"#,
        )
        .expect("parse event");

        match event {
            ServerEvent::ConversationItemInputAudioTranscriptionCompleted {
                item_id,
                content_index,
                transcript,
            } => {
                assert_eq!(item_id, "item-123");
                assert_eq!(content_index, 0);
                assert_eq!(transcript, "hello world");
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
