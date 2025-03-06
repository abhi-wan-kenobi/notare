mod deepgram;
mod errors;

pub use errors::*;

#[cfg(feature = "realtime")]
pub mod realtime;

#[cfg(feature = "recorded")]
pub mod recorded;
