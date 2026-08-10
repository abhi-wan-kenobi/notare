pub mod batch;
pub mod batch_sse;
pub mod batch_stream;
#[cfg(feature = "openapi")]
pub mod openapi;
pub mod progress;
pub mod stream;

#[cfg(feature = "openapi")]
pub use openapi::openapi;
pub use progress::{InferencePhase, InferenceProgress};

#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            PartialEq,
            Debug,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[schemars(deny_unknown_fields)]
        $item
    };
}

// TODO: this is legacy format, but it works, and we already stored them in user db
common_derives! {
    #[derive(Default)]
    pub struct Word2 {
        pub text: String,
        pub speaker: Option<SpeakerIdentity>,
        pub confidence: Option<f32>,
        pub start_ms: Option<u64>,
        pub end_ms: Option<u64>,
    }
}

impl From<stream::Word> for Word2 {
    fn from(word: stream::Word) -> Self {
        Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|s| SpeakerIdentity::Unassigned { index: s as u8 }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        }
    }
}

impl From<batch::Word> for Word2 {
    fn from(word: batch::Word) -> Self {
        Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|s| SpeakerIdentity::Unassigned { index: s as u8 }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        }
    }
}

common_derives! {
    #[serde(tag = "type", content = "value")]
    pub enum SpeakerIdentity {
        #[serde(rename = "unassigned")]
        Unassigned { index: u8 },
        #[serde(rename = "assigned")]
        Assigned { id: String, label: String },
    }
}

common_derives! {
    #[derive(Default)]
    pub struct ListenOutputChunk {
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
        pub meta: Option<serde_json::Value>,
        pub words: Vec<Word2>,
    }
}

common_derives! {
    #[serde(tag = "type", content = "value")]
    pub enum ListenInputChunk {
        #[serde(rename = "audio")]
        Audio {
            #[serde(serialize_with = "serde_bytes::serialize")]
            data: Vec<u8>,
        },
        #[serde(rename = "dual_audio")]
        DualAudio {
            #[serde(serialize_with = "serde_bytes::serialize")]
            mic: Vec<u8>,
            #[serde(serialize_with = "serde_bytes::serialize")]
            speaker: Vec<u8>,
        },
        #[serde(rename = "end")]
        End,
    }
}

#[derive(
    PartialEq,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub enum MixedMessage<A, C> {
    Audio(A),
    Control(C),
}

// https://github.com/deepgram/deepgram-rust-sdk/blob/d2f2723/src/listen/websocket.rs#L772-L778
common_derives! {
    #[serde(tag = "type")]
    pub enum ControlMessage {
        Finalize,
        KeepAlive,
        CloseStream,
    }
}

common_derives! {
    pub struct ListenParams {
        #[serde(default)]
        pub model: Option<String>,
        #[serde(default = "ListenParams::default_channels")]
        pub channels: u8,
        #[serde(default = "ListenParams::default_sample_rate")]
        pub sample_rate: u32,
        // https://docs.rs/axum-extra/0.10.1/axum_extra/extract/struct.Query.html#example-1
        #[serde(default, alias = "language")]
        #[cfg_attr(feature = "openapi", schema(value_type = Vec<String>))]
        pub languages: Vec<hypr_language::Language>,
        #[serde(default)]
        pub keywords: Vec<String>,
        #[serde(default)]
        pub num_speakers: Option<u32>,
        #[serde(default)]
        pub min_speakers: Option<u32>,
        #[serde(default)]
        pub max_speakers: Option<u32>,
        #[serde(default)]
        #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
        pub custom_query: Option<std::collections::HashMap<String, String>>,
    }
}

impl Default for ListenParams {
    fn default() -> Self {
        Self {
            model: None,
            channels: Self::default_channels(),
            sample_rate: Self::default_sample_rate(),
            languages: Vec::new(),
            keywords: Vec::new(),
            num_speakers: None,
            min_speakers: None,
            max_speakers: None,
            custom_query: None,
        }
    }
}

impl ListenParams {
    fn default_channels() -> u8 {
        1
    }

    fn default_sample_rate() -> u32 {
        16000
    }

    /// The `custom_query` key a dictation client sets to request the server's
    /// dictation chunking profile (prompt redemption + a hard max-chunk cut)
    /// instead of the meeting/`speech` profile. Absent leaves meeting behavior
    /// byte-identical.
    ///
    /// Defined here — the single crate both the dictation plugin (which *sends*
    /// this) and `transcribe-core` (which *reads* it) depend on — so a rename
    /// can never silently desync the two sides and regress the Windows D3
    /// stall (dictation falling back to the 20s meeting profile that crashes
    /// Parakeet on DirectML). Guarded by the WS-0 contract test.
    pub const CHUNK_PROFILE_QUERY_KEY: &'static str = "chunk_profile";
    pub const CHUNK_PROFILE_DICTATION: &'static str = "dictation";
    pub const REDEMPTION_TIME_QUERY_KEY: &'static str = "redemption_time_ms";

    /// Build the `custom_query` a dictation session must send: the dictation
    /// chunking profile + its redemption window. Both the plugin and any test
    /// build the wire query through here, so the contract has one source of
    /// truth.
    pub fn dictation_custom_query(
        redemption_time_ms: u64,
    ) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::from([
            (
                Self::REDEMPTION_TIME_QUERY_KEY.to_string(),
                redemption_time_ms.to_string(),
            ),
            (
                Self::CHUNK_PROFILE_QUERY_KEY.to_string(),
                Self::CHUNK_PROFILE_DICTATION.to_string(),
            ),
        ])
    }

    /// True when these params request the dictation chunking profile.
    pub fn is_dictation(&self) -> bool {
        self.custom_query
            .as_ref()
            .and_then(|q| q.get(Self::CHUNK_PROFILE_QUERY_KEY))
            .map(|v| v == Self::CHUNK_PROFILE_DICTATION)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod dictation_contract_tests {
    use super::*;

    // WS-0 (2026-08-06): the Windows D3 stall silently regresses if the
    // dictation query the plugin sends ever stops matching what the server
    // reads. This locks the round-trip: the client-built query MUST make
    // `is_dictation()` true. If a refactor renames the key on one side, this
    // fails instead of shipping a 20s-profile crash to the field again.
    #[test]
    fn client_dictation_query_is_recognized_as_dictation() {
        let params = ListenParams {
            custom_query: Some(ListenParams::dictation_custom_query(400)),
            ..Default::default()
        };
        assert!(
            params.is_dictation(),
            "the query the dictation client sends must select the dictation profile"
        );
        let q = params.custom_query.unwrap();
        assert_eq!(
            q.get(ListenParams::REDEMPTION_TIME_QUERY_KEY).map(String::as_str),
            Some("400")
        );
    }

    #[test]
    fn absent_or_other_profile_is_not_dictation() {
        assert!(!ListenParams::default().is_dictation(), "absent => meeting");

        let other = ListenParams {
            custom_query: Some(std::collections::HashMap::from([(
                ListenParams::CHUNK_PROFILE_QUERY_KEY.to_string(),
                "speech".to_string(),
            )])),
            ..Default::default()
        };
        assert!(!other.is_dictation(), "any other value => meeting");
    }
}
