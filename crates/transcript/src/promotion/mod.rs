use crate::types::RawWord;

/// Decides whether a partial word should be promoted to final status before
/// the ASR provider sends an explicit `is_final` response.
///
/// This is the hook for handling providers that do not reliably send final
/// words — some only send partials, and confirmed partials are the only signal
/// available. The policy runs after every partial update; returning `true`
/// promotes the word immediately, so choose thresholds conservatively.
pub trait PromotionPolicy: Send + Sync {
    fn should_promote(&self, word: &RawWord, consecutive_seen: u32) -> bool;
}

/// Never auto-promote partials. This is the default — partials are only
/// finalized when the provider sends `is_final` or on session flush.
pub struct NeverPromote;

impl PromotionPolicy for NeverPromote {
    fn should_promote(&self, _word: &RawWord, _consecutive_seen: u32) -> bool {
        false
    }
}

/// Promote a partial once it has appeared with identical text and start time
/// in at least `n` consecutive partial responses.
pub struct AfterNSeen {
    pub n: u32,
}

impl PromotionPolicy for AfterNSeen {
    fn should_promote(&self, _word: &RawWord, consecutive_seen: u32) -> bool {
        consecutive_seen >= self.n
    }
}
