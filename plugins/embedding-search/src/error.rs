use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Embedding(#[from] hypr_text_embedding::Error),
    #[error(transparent)]
    Schema(#[from] hypr_db_app::AppSchemaError),
    #[error("embedder background task failed: {0}")]
    Join(String),
    #[error("model download failed: {0}")]
    Download(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("model artifact {name} integrity mismatch: expected {expected}, got {actual}")]
    Integrity {
        name: String,
        expected: String,
        actual: String,
    },
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
