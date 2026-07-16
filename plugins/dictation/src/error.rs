use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not supported on this platform")]
    Unsupported,
    #[error("dictation orb window error: {0}")]
    OrbWindow(String),
    #[error("audio capture error: {0}")]
    Audio(String),
    #[error("dictation session error: {0}")]
    Session(String),
    #[error("text injection error: {0}")]
    Inject(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
