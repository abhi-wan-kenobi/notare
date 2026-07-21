/// A diarized speaker span (from the diarization engine), times in ms.
#[derive(Debug, Clone, Copy)]
pub struct SpeakerSpan {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker_index: i32,
}

/// A transcript word to be labeled, identified by its stable id.
#[derive(Debug, Clone)]
pub struct AlignableWord {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// For each word, pick the speaker whose span has the greatest temporal overlap
/// with the word. If a word overlaps no span, fall back to the span whose
/// midpoint is nearest the word's midpoint (diarization usually covers the whole
/// audio, but guard against gaps). Returns `(word_id, speaker_index)` for every
/// input word, in input order. Empty `spans` -> empty result.
pub fn align_words_to_speakers(
    words: &[AlignableWord],
    spans: &[SpeakerSpan],
) -> Vec<(String, i32)> {
    if spans.is_empty() {
        return Vec::new();
    }

    words
        .iter()
        .map(|word| (word.id.clone(), best_speaker(word, spans)))
        .collect()
}

/// Collapse speaker-label runs shorter than `min_run` words into the surrounding
/// speaker, in place.
///
/// Local per-word diarization can flip the speaker for a single word near a span
/// boundary. Left unsmoothed, each flip starts a new transcript segment, so a
/// long meeting with jittery boundaries produces thousands of one-word segments
/// — pathological for segment consolidation (`consolidate_micro_segments`) and
/// for the (unvirtualized) render, freezing the UI. This is a hysteresis filter:
/// a candidate speaker only "wins" once it has held for `min_run` consecutive
/// words; shorter blips keep the currently-committed speaker. `min_run <= 1` is a
/// no-op. Order-sensitive — expects words in transcript (temporal) order, which
/// is how `align_words_to_speakers` returns them.
pub fn smooth_speaker_runs(assignments: &mut [(String, i32)], min_run: usize) {
    if min_run <= 1 || assignments.len() < 2 {
        return;
    }

    let mut committed = assignments[0].1;
    let mut pending_speaker: Option<i32> = None;
    let mut pending: Vec<usize> = Vec::new();

    let absorb = |assignments: &mut [(String, i32)], pending: &mut Vec<usize>, committed: i32| {
        for &j in pending.iter() {
            assignments[j].1 = committed;
        }
        pending.clear();
    };

    for i in 0..assignments.len() {
        let s = assignments[i].1;
        if s == committed {
            // Back to the committed speaker: the pending run was a short blip —
            // rewrite it as committed.
            absorb(assignments, &mut pending, committed);
            pending_speaker = None;
        } else if pending_speaker == Some(s) {
            pending.push(i);
            if pending.len() >= min_run {
                // The candidate has held long enough — accept the switch; the
                // run keeps its own labels (already `s`).
                committed = s;
                pending.clear();
                pending_speaker = None;
            }
        } else {
            // A third speaker before the pending run qualified: the old pending
            // was too short — absorb it, then start a fresh run here.
            absorb(assignments, &mut pending, committed);
            pending.push(i);
            pending_speaker = Some(s);
        }
    }
    // A trailing run that never reached `min_run` is a blip too.
    absorb(assignments, &mut pending, committed);
}

fn best_speaker(word: &AlignableWord, spans: &[SpeakerSpan]) -> i32 {
    let word_mid = midpoint(word.start_ms, word.end_ms);

    let mut best: Option<Best> = None;
    for (idx, span) in spans.iter().enumerate() {
        let overlap = overlap_ms(word, span);
        let span_mid = midpoint(span.start_ms, span.end_ms);
        let mid_dist = word_mid.abs_diff(span_mid);

        let score = BestScore {
            overlap,
            mid_dist,
            start_ms: span.start_ms,
            slice_idx: idx,
        };

        best = Some(match best {
            None => Best {
                speaker_index: span.speaker_index,
                score,
            },
            Some(cur) if score.better_than(&cur.score) => Best {
                speaker_index: span.speaker_index,
                score,
            },
            Some(cur) => cur,
        });
    }

    best.map(|b| b.speaker_index)
        .unwrap_or(spans[0].speaker_index)
}

#[derive(Clone, Copy)]
struct BestScore {
    overlap: u64,
    mid_dist: u64,
    start_ms: u64,
    slice_idx: usize,
}

#[derive(Clone, Copy)]
struct Best {
    speaker_index: i32,
    score: BestScore,
}

impl BestScore {
    /// True if `self` should replace the current best.
    ///
    /// Primary key: greatest overlap, but only spans whose overlap is non-zero
    /// (i.e. actually touching the word) win on the overlap axis. Once we are in
    /// the zero-overlap fallback, the deciding key switches to nearest midpoint
    /// distance, then earlier span (lower `start_ms`, then lower slice index).
    fn better_than(&self, cur: &BestScore) -> bool {
        let self_touches = self.overlap > 0;
        let cur_touches = cur.overlap > 0;

        match (cur_touches, self_touches) {
            // Current is a real overlap; a non-overlapping span can never beat it.
            (true, false) => false,
            // Current is a fallback; any real overlap wins immediately.
            (false, true) => true,
            // Both overlap: pick the larger overlap. Ties (equal overlap) defer to
            // nearest midpoint, then earlier span.
            (true, true) => {
                if self.overlap != cur.overlap {
                    return self.overlap > cur.overlap;
                }
                self.wins_tiebreaker(cur)
            }
            // Neither overlaps: nearest midpoint, then earlier span.
            (false, false) => {
                if self.overlap != cur.overlap {
                    return self.overlap > cur.overlap;
                }
                self.wins_tiebreaker(cur)
            }
        }
    }

    fn wins_tiebreaker(&self, cur: &BestScore) -> bool {
        if self.mid_dist != cur.mid_dist {
            return self.mid_dist < cur.mid_dist;
        }
        if self.start_ms != cur.start_ms {
            return self.start_ms < cur.start_ms;
        }
        self.slice_idx < cur.slice_idx
    }
}

fn overlap_ms(word: &AlignableWord, span: &SpeakerSpan) -> u64 {
    let lo = word.start_ms.max(span.start_ms);
    let hi = word.end_ms.min(span.end_ms);
    hi.saturating_sub(lo)
}

fn width(start_ms: u64, end_ms: u64) -> u64 {
    end_ms.saturating_sub(start_ms)
}

fn midpoint(start_ms: u64, end_ms: u64) -> u64 {
    start_ms + (width(start_ms, end_ms) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(id: &str, start_ms: u64, end_ms: u64) -> AlignableWord {
        AlignableWord {
            id: id.to_string(),
            start_ms,
            end_ms,
        }
    }

    fn span(start_ms: u64, end_ms: u64, speaker_index: i32) -> SpeakerSpan {
        SpeakerSpan {
            start_ms,
            end_ms,
            speaker_index,
        }
    }

    #[test]
    fn empty_spans_returns_empty() {
        let words = [word("w0", 0, 100), word("w1", 100, 200)];
        assert_eq!(
            align_words_to_speakers(&words, &[]),
            Vec::<(String, i32)>::new()
        );
    }

    #[test]
    fn empty_words_returns_empty() {
        let spans = [span(0, 1000, 0)];
        assert_eq!(
            align_words_to_speakers(&[], &spans),
            Vec::<(String, i32)>::new()
        );
    }

    #[test]
    fn single_speaker_assigns_all_words_to_it() {
        let words = [word("a", 0, 100), word("b", 200, 300), word("c", 500, 600)];
        let spans = [span(0, 10_000, 3)];

        let result = align_words_to_speakers(&words, &spans);
        assert_eq!(
            result,
            vec![
                ("a".to_string(), 3),
                ("b".to_string(), 3),
                ("c".to_string(), 3),
            ]
        );
    }

    #[test]
    fn exact_overlap_picks_largest_overlap() {
        // word fully inside span A, far from span B.
        let words = [word("w", 100, 200)];
        let spans = [
            span(50, 400, 0),  // overlap 100
            span(500, 900, 1), // overlap 0, midpoint far
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    #[test]
    fn word_spanning_boundary_assigned_to_larger_overlap_side() {
        // word straddles the boundary between span A and span B, leaning into B.
        let words = [word("w", 900, 1500)];
        let spans = [
            span(0, 1000, 0),    // overlap 100 (900..1000)
            span(1000, 2000, 1), // overlap 500 (1000..1500)
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 1)]
        );
    }

    #[test]
    fn word_spanning_boundary_assigned_to_a_when_a_larger() {
        // Same setup, but word leans into A.
        let words = [word("w", 100, 600)];
        let spans = [
            span(0, 500, 0),    // overlap 400 (100..500)
            span(500, 1000, 1), // overlap 100 (500..600)
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    #[test]
    fn gap_word_falls_back_to_nearest_midpoint() {
        // word sits in a gap between two spans; no overlap with either.
        let words = [word("w", 1100, 1200)]; // midpoint 1150
        let spans = [
            span(0, 1000, 0),    // midpoint 500, dist 650
            span(1300, 2000, 1), // midpoint 1650, dist 500 -> nearest
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 1)]
        );
    }

    #[test]
    fn gap_word_nearest_to_earlier_span_when_midpoints_tie() {
        // midpoints equidistant -> prefer earlier span (lower start_ms).
        let words = [word("w", 1100, 1200)]; // midpoint 1150
        let spans = [
            span(0, 1000, 0),    // midpoint 500, dist 650
            span(1300, 2000, 1), // midpoint 1650, dist 500
            span(900, 1000, 2),  // midpoint 950, dist 200 -> nearest
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 2)]
        );
    }

    #[test]
    fn overlap_tie_prefers_earlier_span() {
        // Two spans give identical overlap and identical midpoint distance.
        // Both span [0,200] and [0,200]; first by slice order wins.
        let words = [word("w", 50, 150)];
        let spans = [span(0, 200, 0), span(0, 200, 1)];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    #[test]
    fn overlap_tie_breaks_by_lower_start_ms() {
        // Equal overlap, equal midpoint distance, but A starts earlier.
        // Spans: A=[0,200] mid 100, B=[50,250] mid 150. Word=[0,200].
        // overlap A = 200, overlap B = 150 -> not a tie. Redesign for true tie.
        // Use word that yields equal overlap with both but different start.
        // A=[0,100] mid 50, B=[200,300] mid 250, word=[50,250]: overlap A=50, B=50,
        // mid dist A=|150-50|=100, B=|150-250|=100 -> tie. Earlier start (A) wins.
        let words = [word("w", 50, 250)];
        let spans = [span(0, 100, 0), span(200, 300, 1)];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    #[test]
    fn preserves_input_order() {
        let words = [word("c", 500, 600), word("a", 0, 100), word("b", 200, 300)];
        let spans = [span(0, 10_000, 7)];

        let result = align_words_to_speakers(&words, &spans);
        assert_eq!(
            result.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
        assert!(result.iter().all(|(_, sp)| *sp == 7));
    }

    #[test]
    fn handles_inverted_word_bounds_defensively() {
        // end_ms < start_ms -> width 0, midpoint == start_ms. Should not panic.
        let words = [word("w", 500, 100)];
        let spans = [span(0, 10_000, 2)];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 2)]
        );
    }

    #[test]
    fn zero_overlap_with_some_real_overlaps_loses() {
        // Span A overlaps, span B does not but has a closer midpoint.
        // Real overlap must win over the closer-but-disjoint fallback.
        let words = [word("w", 50, 150)]; // midpoint 100
        let spans = [
            span(140, 1000, 0), // overlap 10, midpoint 570, dist 470
            span(0, 40, 1),     // overlap 0, midpoint 20, dist 80 (closer)
        ];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    #[test]
    fn adjacent_spans_no_gap_word_uses_overlap() {
        // Word touches only the right span at a single point: overlap = 0 for both
        // boundary spans? End-exclusive: word=[0,100], spanA=[0,100] overlap 100,
        // spanB=[100,200] overlap 0 -> A wins.
        let words = [word("w", 0, 100)];
        let spans = [span(0, 100, 0), span(100, 200, 1)];

        assert_eq!(
            align_words_to_speakers(&words, &spans),
            vec![("w".to_string(), 0)]
        );
    }

    fn seq(speakers: &[i32]) -> Vec<(String, i32)> {
        speakers
            .iter()
            .enumerate()
            .map(|(i, &s)| (format!("w{i}"), s))
            .collect()
    }

    fn speakers_of(assignments: &[(String, i32)]) -> Vec<i32> {
        assignments.iter().map(|(_, s)| *s).collect()
    }

    #[test]
    fn smoothing_absorbs_isolated_single_word_flip() {
        let mut a = seq(&[0, 0, 1, 0, 0]);
        smooth_speaker_runs(&mut a, 3);
        assert_eq!(speakers_of(&a), vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn smoothing_keeps_a_real_run() {
        let mut a = seq(&[0, 0, 0, 1, 1, 1, 0, 0, 0]);
        smooth_speaker_runs(&mut a, 3);
        assert_eq!(speakers_of(&a), vec![0, 0, 0, 1, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn smoothing_collapses_alternating_singletons() {
        // No run reaches length 3, so everything holds the first speaker —
        // exactly the pathological input that used to explode into 1-word segments.
        let mut a = seq(&[0, 1, 0, 1, 0, 1]);
        smooth_speaker_runs(&mut a, 3);
        assert_eq!(speakers_of(&a), vec![0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn smoothing_switches_once_new_speaker_sustains() {
        // A short blip is absorbed, but a sustained run of the new speaker wins.
        let mut a = seq(&[0, 0, 1, 0, 1, 1, 1, 1]);
        smooth_speaker_runs(&mut a, 3);
        assert_eq!(speakers_of(&a), vec![0, 0, 0, 0, 1, 1, 1, 1]);
    }

    #[test]
    fn smoothing_min_run_one_is_a_noop() {
        let mut a = seq(&[0, 1, 0, 2, 0]);
        smooth_speaker_runs(&mut a, 1);
        assert_eq!(speakers_of(&a), vec![0, 1, 0, 2, 0]);
    }
}
