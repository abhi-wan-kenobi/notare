use serde::{ser::Serializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("calendar access denied")]
    CalendarAccessDenied,
    #[error("contact access denied")]
    ContactAccessDenied,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
