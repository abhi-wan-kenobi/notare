use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

/// A device-pairing offer opened as `notare://pair#<offer>`.
#[derive(Clone, Serialize, Deserialize, Type)]
pub struct PairSearch {
    /// The complete `notare://pair#<offer>` link. The offer carries the pairing
    /// secret, so it is never logged.
    pub link: String,
}

impl fmt::Debug for PairSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairSearch")
            .field("link", &"[REDACTED]")
            .finish()
    }
}
