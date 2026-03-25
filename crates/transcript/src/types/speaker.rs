#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerHintData {
    ProviderSpeakerIndex {
        speaker_index: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        channel: Option<i32>,
    },
    UserSpeakerAssignment {
        human_id: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WordRef {
    FinalWordId(String),
    RuntimeIndex(usize),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RuntimeSpeakerHint {
    pub target: WordRef,
    pub data: SpeakerHintData,
}
