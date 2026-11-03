use std::sync::Mutex;

use crate::types::PairSearch;

/// The latest pairing link the OS handed to the app that the UI has not
/// consumed yet.
///
/// Cold start (the app was launched by the link) delivers the URL before any
/// webview exists to listen for the event, so it is parked here and the UI
/// drains it on mount. Warm links are parked too and the event is only a
/// wake-up: draining is the single consumption point, so a link is neither
/// lost nor handled twice.
#[derive(Default)]
pub struct PendingPair(Mutex<Option<PairSearch>>);

impl PendingPair {
    pub fn set(&self, search: PairSearch) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(search);
    }

    pub fn take(&self) -> Option<PairSearch> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}
