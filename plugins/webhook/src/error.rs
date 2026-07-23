use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] tauri_plugin_store2::Error),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Task(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
