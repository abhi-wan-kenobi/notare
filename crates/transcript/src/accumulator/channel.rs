use crate::id::IdGenerator;
use crate::promotion::PromotionPolicy;
use crate::types::{RawWord, SpeakerHint, TranscriptWord};

use super::FlushMode;
use super::words::{
    PartialEntry, dedup, finalize_words, splice_partials, stitch, strip_overlap_entries,
};

pub(super) struct ChannelState {
    watermark: i64,
    held: Option<RawWord>,
    partials: Vec<PartialEntry>,
}

impl ChannelState {
    pub(super) fn new() -> Self {
        Self {
            watermark: 0,
            held: None,
            partials: Vec::new(),
        }
    }

    pub(super) fn partials(&self) -> impl Iterator<Item = &RawWord> {
        self.partials.iter().map(|e| &e.word)
    }

    pub(super) fn partial_entries(&self) -> impl Iterator<Item = &PartialEntry> {
        self.partials.iter()
    }

    pub(super) fn apply_final(
        &mut self,
        words: Vec<RawWord>,
        id_gen: &mut dyn IdGenerator,
    ) -> (Vec<TranscriptWord>, Vec<SpeakerHint>) {
        let response_end = words.last().map_or(0, |w| w.end_ms);
        let new_words = dedup(words, self.watermark);

        if new_words.is_empty() {
            return (vec![], vec![]);
        }

        self.watermark = response_end;
        self.partials = strip_overlap_entries(std::mem::take(&mut self.partials), response_end);

        let (emitted, held) = stitch(self.held.take(), new_words);
        self.held = held;
        finalize_words(emitted, id_gen)
    }

    /// Update partials and run the promotion policy.
    ///
    /// Returns any words that the policy promoted to final this cycle.
    /// For [`crate::promotion::NeverPromote`] (the default) this is always
    /// empty — partials are only finalized via `apply_final` or `drain`.
    pub(super) fn apply_partial(
        &mut self,
        words: Vec<RawWord>,
        policy: &dyn PromotionPolicy,
        id_gen: &mut dyn IdGenerator,
    ) -> (Vec<TranscriptWord>, Vec<SpeakerHint>) {
        self.partials = splice_partials(&self.partials, words);

        let (promoted_entries, remaining): (Vec<_>, Vec<_>) = std::mem::take(&mut self.partials)
            .into_iter()
            .partition(|e| policy.should_promote(&e.word, e.consecutive_seen));

        self.partials = remaining;

        let promoted: Vec<RawWord> = promoted_entries.into_iter().map(|e| e.word).collect();
        if promoted.is_empty() {
            return (vec![], vec![]);
        }

        if let Some(last) = promoted.last() {
            self.watermark = self.watermark.max(last.end_ms);
        }

        finalize_words(promoted, id_gen)
    }

    /// Drain remaining state at session end.
    ///
    /// - [`FlushMode::DrainAll`]: promotes the held word and all partials,
    ///   regardless of stability. Use at hard session end.
    /// - [`FlushMode::PromotableOnly`]: promotes the held word (it was already
    ///   confirmed by an `is_final` response) and only those partials that
    ///   satisfy the promotion policy. Remaining partials are silently dropped.
    pub(super) fn drain(
        &mut self,
        mode: FlushMode,
        policy: &dyn PromotionPolicy,
        id_gen: &mut dyn IdGenerator,
    ) -> (Vec<TranscriptWord>, Vec<SpeakerHint>) {
        let mut raw: Vec<RawWord> = self.held.take().into_iter().collect();

        match mode {
            FlushMode::DrainAll => {
                raw.extend(
                    std::mem::take(&mut self.partials)
                        .into_iter()
                        .map(|e| e.word),
                );
            }
            FlushMode::PromotableOnly => {
                raw.extend(
                    std::mem::take(&mut self.partials)
                        .into_iter()
                        .filter(|e| policy.should_promote(&e.word, e.consecutive_seen))
                        .map(|e| e.word),
                );
            }
        }

        finalize_words(raw, id_gen)
    }
}
