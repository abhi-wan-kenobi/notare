use std::time::Duration;

use hypr_audio_chunking::{AudioChunk, Chunker, SpeechChunker, SpeechChunkingConfig};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

const DEFAULT_SPEECH_REDEMPTION_TIME: Duration = Duration::from_millis(150);
/// Universal ceiling on samples handed to an engine in one call (25s). No batch
/// chunk is ever larger, whatever the engine, because Voxtral/libmtmd has a
/// fixed 30s audio window — a longer chunk is silently truncated (dropped
/// transcript). whisper/parakeet tolerate oversize input, so 25s keeps them
/// correct too.
///
/// An engine may need a SMALLER cap: Parakeet-on-Windows-DirectML aborts the
/// process on buffers past ~20s (D3), so it lowers its cap to 15s via
/// [`SttEngineSession::max_samples_per_call`]. The streaming path already honors
/// that per VAD chunk (see `service::streaming::build_transcription_streams`);
/// the batch path honors it too by taking `min(engine cap, MAX_CHUNK_SAMPLES)`
/// — otherwise a re-transcription of a recorded file would hand Parakeet a 25s
/// buffer and crash exactly like dictation did before the fix.
pub(crate) const MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 25;

/// Chunk one channel's audio for batch transcription, windowed to the engine's
/// own per-call limit (never above [`MAX_CHUNK_SAMPLES`]). Pass the session's
/// [`SttEngineSession::max_samples_per_call`]; `usize::MAX` selects the ceiling.
pub fn chunk_channel_audio<E>(
    samples: &[f32],
    engine_max_samples: usize,
) -> Result<Vec<AudioChunk>, E>
where
    E: From<hypr_audio_chunking::Error>,
{
    let mut chunker =
        SpeechChunker::new(SpeechChunkingConfig::speech(DEFAULT_SPEECH_REDEMPTION_TIME))?;
    Ok(chunk_channel_audio_with(
        samples,
        &mut chunker,
        engine_max_samples,
    )?)
}

fn chunk_channel_audio_with<C>(
    samples: &[f32],
    chunker: &mut C,
    engine_max_samples: usize,
) -> Result<Vec<AudioChunk>, C::Error>
where
    C: Chunker,
{
    // Never exceed the engine's own limit nor the universal ceiling. The
    // `.max(1)` mirrors the streaming path (`TranscribeChannelStream`): a cap of
    // 0 would reach `slice::chunks(0)` in the split pass below and panic. No
    // engine returns 0 today, but the two cap sites must defend identically.
    let max_chunk_samples = engine_max_samples.min(MAX_CHUNK_SAMPLES).max(1);
    let chunks = chunker.chunk(samples, TARGET_SAMPLE_RATE)?;
    let total = samples.len();

    // --- Pack adjacent VAD chunks up toward MAX_CHUNK_SAMPLES ----------------
    // whisper.cpp's GPU encoder always computes over a fixed 30s window, so its
    // per-call cost is ~constant regardless of chunk length. The VAD produces one
    // chunk per utterance/breath, so a long recording became *hundreds* of tiny
    // calls — each paying the full fixed cost — collapsing batch throughput to
    // ~1x even though the GPU can do ~8x. Group adjacent chunks whose combined
    // span stays within the cap and transcribe the ORIGINAL contiguous audio for
    // that span (including the inter-utterance silence), so each whisper call
    // amortizes the fixed cost over ~25s and word timings still map to the real
    // timeline (offsets are relative to the packed chunk's sample_start).
    let mut packed: Vec<AudioChunk> = Vec::new();
    let mut group_start: Option<usize> = None;
    let mut group_end: usize = 0;
    for chunk in &chunks {
        match group_start {
            Some(gs) if chunk.sample_end.saturating_sub(gs) <= max_chunk_samples => {
                group_end = chunk.sample_end;
            }
            Some(gs) => {
                let end = group_end.min(total);
                if end > gs {
                    packed.push(AudioChunk {
                        samples: samples[gs..end].to_vec(),
                        sample_start: gs,
                        sample_end: end,
                    });
                }
                group_start = Some(chunk.sample_start);
                group_end = chunk.sample_end;
            }
            None => {
                group_start = Some(chunk.sample_start);
                group_end = chunk.sample_end;
            }
        }
    }
    if let Some(gs) = group_start {
        let end = group_end.min(total);
        if end > gs {
            packed.push(AudioChunk {
                samples: samples[gs..end].to_vec(),
                sample_start: gs,
                sample_end: end,
            });
        }
    }

    // --- Split any packed group still over the cap (a single >25s utterance) --
    let mut normalized = Vec::new();
    for chunk in packed {
        if chunk.samples.len() <= max_chunk_samples {
            normalized.push(chunk);
            continue;
        }

        for (index, window) in chunk.samples.chunks(max_chunk_samples).enumerate() {
            let sample_start = chunk.sample_start + index * max_chunk_samples;
            let sample_end = sample_start + window.len();
            normalized.push(AudioChunk {
                samples: window.to_vec(),
                sample_start,
                sample_end,
            });
        }
    }

    tracing::info!(
        chunk_count = normalized.len(),
        chunk_durations_ms = ?normalized
            .iter()
            .map(|chunk| (chunk.sample_end - chunk.sample_start) * 1000 / TARGET_SAMPLE_RATE as usize)
            .collect::<Vec<_>>(),
        "audio_chunking_complete"
    );

    Ok(normalized)
}

pub fn split_resampled_channels(samples: &[f32], channel_count: usize) -> Vec<Vec<f32>> {
    if channel_count <= 1 {
        return vec![samples.to_vec()];
    }

    hypr_audio_utils::deinterleave(samples, channel_count)
}

pub fn channel_duration_sec(samples: &[f32]) -> f64 {
    samples.len() as f64 / TARGET_SAMPLE_RATE as f64
}

#[cfg(test)]
mod tests {
    use hypr_audio_chunking::{AudioChunk, Chunker};

    use super::*;
    use crate::{initial_resolved_until, next_resolved_until};

    struct FakeChunker {
        chunks: Vec<AudioChunk>,
    }

    impl Chunker for FakeChunker {
        type Error = std::convert::Infallible;

        fn chunk(
            &mut self,
            _samples: &[f32],
            _sample_rate: u32,
        ) -> Result<Vec<AudioChunk>, Self::Error> {
            Ok(self.chunks.clone())
        }
    }

    #[test]
    fn empty_audio_marks_channel_complete() {
        let chunks = chunk_channel_audio::<hypr_audio_chunking::Error>(&[], usize::MAX).unwrap();

        assert!(chunks.is_empty());
        assert_eq!(initial_resolved_until(&chunks, 40.0), 40.0);
    }

    #[test]
    fn empty_chunk_lists_mark_channel_complete() {
        let mut chunker = FakeChunker { chunks: Vec::new() };
        let chunks = chunk_channel_audio_with(&[], &mut chunker, usize::MAX).unwrap();

        assert!(chunks.is_empty());
        assert_eq!(initial_resolved_until(&chunks, 40.0), 40.0);
    }

    #[test]
    fn leading_silence_uses_sample_offsets() {
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 3],
                sample_start: TARGET_SAMPLE_RATE as usize * 12,
                sample_end: TARGET_SAMPLE_RATE as usize * 15,
            }],
        };
        let chunks = chunk_channel_audio_with(
            &vec![0.0; TARGET_SAMPLE_RATE as usize * 15],
            &mut chunker,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(initial_resolved_until(&chunks, 40.0), 12.0);
    }

    #[test]
    fn oversized_chunks_are_split_at_generic_limit() {
        let oversized = MAX_CHUNK_SAMPLES + TARGET_SAMPLE_RATE as usize;
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![1.0; oversized],
                sample_start: TARGET_SAMPLE_RATE as usize * 2,
                sample_end: TARGET_SAMPLE_RATE as usize * 2 + oversized,
            }],
        };

        let chunks = chunk_channel_audio_with(
            &vec![0.0; TARGET_SAMPLE_RATE as usize * 2 + oversized],
            &mut chunker,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].sample_start, TARGET_SAMPLE_RATE as usize * 2);
        assert_eq!(
            chunks[0].sample_end,
            TARGET_SAMPLE_RATE as usize * 2 + MAX_CHUNK_SAMPLES
        );
        assert_eq!(chunks[1].sample_start, chunks[0].sample_end);
        assert_eq!(chunks[1].samples.len(), TARGET_SAMPLE_RATE as usize);
    }

    #[test]
    fn batch_path_honors_a_smaller_engine_cap() {
        // Regression for the D3-class batch gap (WS-1, 2026-08-06): the batch
        // path used to split uniformly at the 25s ceiling, so a Windows Parakeet
        // re-transcription got a 25s buffer past its ~20s DirectML crash point.
        // With Parakeet's 15s cap, a single 20s utterance must split into 15s +
        // 5s — never a >15s chunk.
        let parakeet_cap = TARGET_SAMPLE_RATE as usize * 15;
        let utterance = TARGET_SAMPLE_RATE as usize * 20;
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![1.0; utterance],
                sample_start: 0,
                sample_end: utterance,
            }],
        };

        let chunks =
            chunk_channel_audio_with(&vec![0.0; utterance], &mut chunker, parakeet_cap).unwrap();

        assert!(
            chunks.iter().all(|c| c.samples.len() <= parakeet_cap),
            "no batch chunk may exceed the engine's per-call cap"
        );
        assert_eq!(chunks.len(), 2, "20s at a 15s cap => two chunks");
        assert_eq!(chunks[0].samples.len(), parakeet_cap);
        assert_eq!(chunks[1].samples.len(), TARGET_SAMPLE_RATE as usize * 5);
    }

    #[test]
    fn zero_engine_cap_is_clamped_and_does_not_panic() {
        // Regression for the audit finding (kimi + minimax, 2026-08-06): a cap of
        // 0 must be clamped to 1, or the split pass calls `slice::chunks(0)` and
        // panics — the same defense the streaming path already has. Tiny input so
        // the clamped 1-sample windows stay cheap.
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![1.0; 3],
                sample_start: 0,
                sample_end: 3,
            }],
        };
        let chunks = chunk_channel_audio_with(&vec![0.0; 3], &mut chunker, 0).unwrap();
        assert!(chunks.iter().all(|c| !c.samples.is_empty()));
        assert_eq!(chunks.iter().map(|c| c.samples.len()).sum::<usize>(), 3);
    }

    #[test]
    fn resolved_progress_uses_sample_offsets() {
        // Chunks spaced >25s apart so the packing pass keeps them separate.
        let mut chunker = FakeChunker {
            chunks: vec![
                AudioChunk {
                    samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 2],
                    sample_start: TARGET_SAMPLE_RATE as usize * 4,
                    sample_end: TARGET_SAMPLE_RATE as usize * 6,
                },
                AudioChunk {
                    samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 3],
                    sample_start: TARGET_SAMPLE_RATE as usize * 40,
                    sample_end: TARGET_SAMPLE_RATE as usize * 43,
                },
            ],
        };

        let chunks = chunk_channel_audio_with(
            &vec![0.0; TARGET_SAMPLE_RATE as usize * 45],
            &mut chunker,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(initial_resolved_until(&chunks, 45.0), 4.0);
        assert_eq!(next_resolved_until(&chunks, 0, 45.0), 40.0);
        assert_eq!(next_resolved_until(&chunks, 1, 45.0), 45.0);
    }

    #[test]
    fn adjacent_chunks_pack_into_one_call() {
        // Ten 1s utterance chunks within a 25s span must pack into ONE whisper
        // call (the fixed-30s-window amortization fix). Each is offset 2s apart,
        // so the packed chunk spans 0..19s of the original contiguous audio.
        let chunks: Vec<AudioChunk> = (0..10usize)
            .map(|i| AudioChunk {
                samples: vec![0.0; TARGET_SAMPLE_RATE as usize],
                sample_start: TARGET_SAMPLE_RATE as usize * 2 * i,
                sample_end: TARGET_SAMPLE_RATE as usize * 2 * i + TARGET_SAMPLE_RATE as usize,
            })
            .collect();
        let mut chunker = FakeChunker { chunks };

        let out = chunk_channel_audio_with(
            &vec![0.0; TARGET_SAMPLE_RATE as usize * 20],
            &mut chunker,
            usize::MAX,
        )
        .unwrap();

        assert_eq!(
            out.len(),
            1,
            "adjacent short chunks should pack into one call"
        );
        assert_eq!(out[0].sample_start, 0);
        assert_eq!(out[0].sample_end, TARGET_SAMPLE_RATE as usize * 19);
        assert_eq!(out[0].samples.len(), TARGET_SAMPLE_RATE as usize * 19);
    }
}
