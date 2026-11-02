use serde::{Serialize, ser::Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("port {0} is already in use")]
    PortInUse(u16),

    #[error("invalid port {0}: port cannot be 0 or a privileged port (< 1024)")]
    InvalidPort(u16),

    #[error("keyring error: {0}")]
    Keyring(String),

    #[error(transparent)]
    Store(#[from] tauri_plugin_store2::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("database error: {0}")]
    Database(String),

    #[error("server error: {0}")]
    Server(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
