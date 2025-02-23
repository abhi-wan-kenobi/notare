#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, schemars::JsonSchema,
        )]
        #[schemars(deny_unknown_fields)]
        $item
    };
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("unknown error")]
    Unknown,
    #[error("connection error")]
    Connection(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("timeout error")]
    Timeout(#[from] tokio::time::error::Elapsed),
}

common_derives! {
    pub enum ListenOutputChunk {
        Transcribe(hypr_db::user::TranscriptChunk),
        Diarize(hypr_db::user::DiarizationChunk),
    }
}

common_derives! {
    pub struct ListenInputChunk {
        #[serde(serialize_with = "serde_bytes::serialize")]
        pub audio: Vec<u8>,
    }
}

pub type DiarizeInputChunk = ListenInputChunk;
pub type TranscribeInputChunk = ListenInputChunk;
pub type DiarizeOutputChunk = hypr_db::user::DiarizationChunk;
pub type TranscribeOutputChunk = hypr_db::user::TranscriptChunk;

common_derives! {
    pub struct ListenParams {
        #[specta(type = String)]
        #[schemars(with = "String")]
        pub language: codes_iso_639::part_1::LanguageCode,
    }
}
