use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error (status {0}): {1}")]
    Api(u16, String),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid signing key: {0}")]
    InvalidSigningKey(String),
    #[error("signing failed: {0}")]
    Signing(String),
    #[error("invalid permissions: {0}")]
    InvalidPermissions(&'static str),
    #[error("invalid api base url")]
    InvalidApiBase,
    #[error("missing token or signing key")]
    MissingToken,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
