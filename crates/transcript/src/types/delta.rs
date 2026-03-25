use super::speaker::RuntimeSpeakerHint;
use super::word::{FinalizedWord, PartialWord};

/// Delta emitted to the frontend after processing.
///
/// The frontend should:
/// 1. Remove words listed in `replaced_ids` from TinyBase
/// 2. Persist `new_words` to TinyBase (honoring `state`)
/// 3. Store `partials` and `partial_hints` in ephemeral state for rendering
///
/// This shape handles all correction flows uniformly:
/// - Normal finalization: `new_words` with `Final`, empty `replaced_ids`
/// - Pending correction submitted: `new_words` with `Pending`, `replaced_ids`
///   pointing at the same words' previous `Final` versions
/// - Correction resolved: `new_words` with `Final` (corrected text),
///   `replaced_ids` pointing at the `Pending` versions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TranscriptDelta {
    pub new_words: Vec<FinalizedWord>,
    pub hints: Vec<RuntimeSpeakerHint>,
    /// IDs of words superseded by `new_words`. Empty for normal finalization.
    pub replaced_ids: Vec<String>,
    /// Current in-progress words across all channels. Global snapshot.
    pub partials: Vec<PartialWord>,
    /// Speaker hints for `partials`, indexed relative to the `partials` snapshot.
    pub partial_hints: Vec<RuntimeSpeakerHint>,
}

impl TranscriptDelta {
    pub fn is_empty(&self) -> bool {
        self.new_words.is_empty()
            && self.replaced_ids.is_empty()
            && self.partials.is_empty()
            && self.partial_hints.is_empty()
    }
}
