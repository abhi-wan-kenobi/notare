use std::collections::HashMap;

use hypr_diarization::DiarizedSegment;
use serde::{Deserialize, Serialize};

/// A transcript word to be labeled by the diarizer, identified by its stable id.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct DiarWordInput {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A stretch of the recording, in milliseconds from the start of the audio file.
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
pub struct AudioSpanMs {
    pub start_ms: u64,
    pub end_ms: u64,
}

/// The diarizer's per-word speaker assignment.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DiarWordSpeaker {
    pub word_id: String,
    pub speaker_index: i32,
}

/// An enrolled voice profile to match diarized speakers against.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct EnrolledProfile {
    pub human_id: String,
    pub embedding: Vec<f32>,
}

/// A diarized speaker matched to an enrolled human (voice recognition).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SpeakerHuman {
    pub speaker_index: i32,
    pub human_id: String,
}

/// Diarization + recognition result: per-word speaker indices, plus any
/// diarized speakers that matched an enrolled voice profile.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct DiarizationResult {
    pub word_speakers: Vec<DiarWordSpeaker>,
    pub speaker_humans: Vec<SpeakerHuman>,
}

/// Cosine-match threshold for recognizing a diarized speaker as an enrolled
/// human. Conservative so an unknown speaker stays "Speaker N" rather than
/// being mislabeled. Tunable later.
const MATCH_THRESHOLD: f32 = 0.5;

const SAMPLE_RATE_HZ: usize = 16_000;

/// Minimum consecutive words a speaker must hold before a label switch is kept.
/// Matches the segment builder's `min_segment_words` so smoothed runs don't
/// immediately become "micro" segments. Suppresses single-word diarization
/// flips that would otherwise fragment the transcript into thousands of
/// one-word segments (a UI-freezing, O(n²) input to segment consolidation).
const MIN_SPEAKER_RUN_WORDS: usize = 3;

/// Run on-device speaker diarization over a recorded audio file: assign each
/// supplied word a speaker index (by timestamp overlap), and — when `enrolled`
/// profiles are supplied — recognize which diarized speakers are known humans.
///
/// Pure compute. The caller persists `word_speakers` as `provider_speaker_index`
/// hints and `speaker_humans` as `user_speaker_assignment` hints — exactly the
/// cloud-provider + manual-assignment paths — so the render/label pipeline is
/// unchanged. Runs on the bundled models (no download).
#[tauri::command]
#[specta::specta]
pub async fn run_diarization(
    audio_path: String,
    num_speakers: Option<i32>,
    words: Vec<DiarWordInput>,
    enrolled: Vec<EnrolledProfile>,
) -> Result<DiarizationResult, String> {
    // Diarization + ONNX inference is blocking/CPU-heavy — keep it off the async
    // runtime's worker threads.
    tokio::task::spawn_blocking(move || {
        diarize_and_align(audio_path, num_speakers, words, enrolled)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Compute a speaker embedding for enrollment. The caller stores it as a voice
/// profile (`voice_profiles` table) against a human. Uses the same bundled
/// embedding model as diarization, so enrolled profiles are directly comparable
/// to diarized speakers.
///
/// `spans` selects one speaker's turns inside a longer recording (a meeting);
/// they are concatenated before embedding. An empty list embeds the whole file
/// (a clip of just that person).
#[tauri::command]
#[specta::specta]
pub async fn compute_voice_embedding(
    audio_path: String,
    spans: Vec<AudioSpanMs>,
) -> Result<Vec<f32>, String> {
    tokio::task::spawn_blocking(move || {
        let mono = decode_to_mono_16k(&audio_path)?;
        let sample = enrollment_sample(mono, &spans)?;
        let mut index = hypr_diarization::VoiceProfileIndex::from_bundled().map_err(stringify)?;
        // Enroll under a throwaway id, then read the stored embedding back.
        index
            .enroll("__probe__".to_string(), &sample)
            .map_err(stringify)?;
        index
            .profiles()
            .into_iter()
            .next()
            .map(|(_, embedding)| embedding)
            .ok_or_else(|| "no embedding produced".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The 16 kHz sample range a millisecond span covers, clamped to the audio.
/// `None` when the span is empty or lies past the end.
fn sample_range(len: usize, start_ms: u64, end_ms: u64) -> Option<std::ops::Range<usize>> {
    let to_sample = |ms: u64| {
        usize::try_from(ms.saturating_mul(SAMPLE_RATE_HZ as u64) / 1000).unwrap_or(usize::MAX)
    };
    let start = to_sample(start_ms).min(len);
    let end = to_sample(end_ms).min(len);
    (start < end).then_some(start..end)
}

fn concat_spans(mono: &[f32], spans: &[AudioSpanMs]) -> Vec<f32> {
    spans
        .iter()
        .filter_map(|span| sample_range(mono.len(), span.start_ms, span.end_ms))
        .flat_map(|range| mono[range].iter().copied())
        .collect()
}

/// Below this much audio the speaker embedding is too noisy to recognize
/// anyone by; the caller is told instead of getting a weak profile.
const MIN_ENROLLMENT_SAMPLES: usize = 3 * SAMPLE_RATE_HZ;
const MAX_ENROLLMENT_SAMPLES: usize = 60 * SAMPLE_RATE_HZ;

/// The audio to embed for enrollment: the requested spans (or the whole file
/// when none are given), capped at [`MAX_ENROLLMENT_SAMPLES`] and refused when
/// shorter than [`MIN_ENROLLMENT_SAMPLES`] (for example spans that lie past the
/// end of the recording).
fn enrollment_sample(mono: Vec<f32>, spans: &[AudioSpanMs]) -> Result<Vec<f32>, String> {
    let mut sample = if spans.is_empty() {
        mono
    } else {
        concat_spans(&mono, spans)
    };
    if sample.len() < MIN_ENROLLMENT_SAMPLES {
        return Err(format!(
            "need at least {} s of speech to learn a voice, got {:.1} s",
            MIN_ENROLLMENT_SAMPLES / SAMPLE_RATE_HZ,
            sample.len() as f32 / SAMPLE_RATE_HZ as f32,
        ));
    }
    sample.truncate(MAX_ENROLLMENT_SAMPLES);
    Ok(sample)
}

fn diarize_and_align(
    audio_path: String,
    num_speakers: Option<i32>,
    words: Vec<DiarWordInput>,
    enrolled: Vec<EnrolledProfile>,
) -> Result<DiarizationResult, String> {
    use hypr_transcript::diar_align::{AlignableWord, SpeakerSpan, align_words_to_speakers};

    let mono = decode_to_mono_16k(&audio_path)?;

    // Diarize into speaker spans, then align them onto the supplied words.
    let mut diarizer = hypr_diarization::Diarizer::from_bundled().map_err(stringify)?;
    let segments = diarizer.diarize(&mono, num_speakers).map_err(stringify)?;

    let spans: Vec<SpeakerSpan> = segments
        .iter()
        .map(|s| SpeakerSpan {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            speaker_index: s.speaker_index,
        })
        .collect();
    let alignable: Vec<AlignableWord> = words
        .into_iter()
        .map(|w| AlignableWord {
            id: w.id,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
        })
        .collect();

    let mut aligned = align_words_to_speakers(&alignable, &spans);
    // Smooth out single-word speaker flips at span boundaries before they become
    // per-word transcript segments. Without this, jittery local diarization on a
    // long meeting produces thousands of one-word segments, which is O(n²) in
    // segment consolidation and freezes the (unvirtualized) transcript render.
    hypr_transcript::diar_align::smooth_speaker_runs(&mut aligned, MIN_SPEAKER_RUN_WORDS);
    let word_speakers = aligned
        .into_iter()
        .map(|(word_id, speaker_index)| DiarWordSpeaker {
            word_id,
            speaker_index,
        })
        .collect();

    let speaker_humans = if enrolled.is_empty() {
        Vec::new()
    } else {
        identify_speakers(&mono, &segments, &enrolled)?
    };

    Ok(DiarizationResult {
        word_speakers,
        speaker_humans,
    })
}

/// For each diarized speaker, embed its longest turn and match it against the
/// enrolled profiles; keep matches above the threshold.
fn identify_speakers(
    mono: &[f32],
    segments: &[DiarizedSegment],
    enrolled: &[EnrolledProfile],
) -> Result<Vec<SpeakerHuman>, String> {
    let (_segmentation, embedding) = hypr_diarization::bundled_model_paths().map_err(stringify)?;
    let profiles: Vec<(String, Vec<f32>)> = enrolled
        .iter()
        .map(|p| (p.human_id.clone(), p.embedding.clone()))
        .collect();
    let mut index = hypr_diarization::VoiceProfileIndex::from_profiles(embedding, profiles)
        .map_err(stringify)?;

    // The longest turn per speaker gives the most reliable embedding.
    let mut longest: HashMap<i32, &DiarizedSegment> = HashMap::new();
    for seg in segments {
        let dur = seg.end_ms.saturating_sub(seg.start_ms);
        longest
            .entry(seg.speaker_index)
            .and_modify(|cur| {
                if dur > cur.end_ms.saturating_sub(cur.start_ms) {
                    *cur = seg;
                }
            })
            .or_insert(seg);
    }

    let mut speakers: Vec<(i32, &DiarizedSegment)> = longest.into_iter().collect();
    speakers.sort_by_key(|(idx, _)| *idx);

    let mut out = Vec::new();
    for (speaker_index, seg) in speakers {
        let Some(range) = sample_range(mono.len(), seg.start_ms, seg.end_ms) else {
            continue;
        };
        if let Some(human_id) = index
            .identify(&mono[range], MATCH_THRESHOLD)
            .map_err(stringify)?
        {
            out.push(SpeakerHuman {
                speaker_index,
                human_id,
            });
        }
    }
    Ok(out)
}

fn decode_to_mono_16k(audio_path: &str) -> Result<Vec<f32>, String> {
    let meta = hypr_audio_utils::audio_file_metadata(audio_path).map_err(stringify)?;
    let source = hypr_audio_utils::source_from_path(audio_path).map_err(stringify)?;
    let resampled = hypr_audio_utils::resample_audio(source, 16_000).map_err(stringify)?;
    let channels = std::num::NonZeroU8::new(meta.channels).unwrap_or(std::num::NonZeroU8::MIN);
    Ok(hypr_audio_utils::mix_down_to_mono(&resampled, channels))
}

fn stringify<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(ms: u64) -> usize {
        (ms as usize) * SAMPLE_RATE_HZ / 1000
    }

    #[test]
    fn sample_range_clamps_to_the_audio() {
        let len = ms(2_000);
        assert_eq!(sample_range(len, 500, 1_000), Some(ms(500)..ms(1_000)));
        assert_eq!(sample_range(len, 1_500, 9_000), Some(ms(1_500)..len));
        assert_eq!(sample_range(len, 2_000, 3_000), None);
        assert_eq!(sample_range(len, 1_000, 1_000), None);
        assert_eq!(sample_range(len, 1_200, 800), None);
        assert_eq!(sample_range(len, u64::MAX - 1, u64::MAX), None);
    }

    #[test]
    fn concat_spans_keeps_only_the_requested_audio_in_order() {
        // Each sample holds its own index, so the output shows exactly which
        // samples were taken.
        let mono: Vec<f32> = (0..ms(3_000)).map(|i| i as f32).collect();
        let spans = [
            AudioSpanMs {
                start_ms: 2_000,
                end_ms: 2_500,
            },
            AudioSpanMs {
                start_ms: 100,
                end_ms: 200,
            },
            AudioSpanMs {
                start_ms: 2_900,
                end_ms: 4_000,
            },
            AudioSpanMs {
                start_ms: 5_000,
                end_ms: 6_000,
            },
        ];

        let out = concat_spans(&mono, &spans);

        let expected: Vec<f32> = (ms(2_000)..ms(2_500))
            .chain(ms(100)..ms(200))
            .chain(ms(2_900)..ms(3_000))
            .map(|i| i as f32)
            .collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn concat_spans_is_empty_when_no_span_overlaps_the_audio() {
        let mono = vec![0.0_f32; ms(1_000)];
        let spans = [AudioSpanMs {
            start_ms: 1_000,
            end_ms: 2_000,
        }];
        assert!(concat_spans(&mono, &spans).is_empty());
    }

    #[test]
    fn enrollment_sample_refuses_spans_that_fall_past_the_recording() {
        // A 5 s recording asked for 0-2 s plus 4-60 s holds only 3 s.
        let mono = vec![0.0_f32; ms(5_000)];
        let spans = [
            AudioSpanMs {
                start_ms: 0,
                end_ms: 2_000,
            },
            AudioSpanMs {
                start_ms: 4_000,
                end_ms: 60_000,
            },
        ];
        assert_eq!(
            enrollment_sample(mono.clone(), &spans).unwrap().len(),
            ms(3_000)
        );

        let short = [AudioSpanMs {
            start_ms: 4_000,
            end_ms: 60_000,
        }];
        let err = enrollment_sample(mono, &short).unwrap_err();
        assert!(err.contains("got 1.0 s"), "{err}");
    }

    #[test]
    fn enrollment_sample_uses_the_whole_file_without_spans() {
        let mono = vec![0.5_f32; ms(4_000)];
        assert_eq!(enrollment_sample(mono.clone(), &[]).unwrap(), mono);
        assert!(enrollment_sample(vec![0.5_f32; ms(2_000)], &[]).is_err());
    }

    #[test]
    fn enrollment_sample_caps_imported_clips_at_one_minute() {
        let mono: Vec<f32> = (0..ms(75_000)).map(|sample| sample as f32).collect();
        let sample = enrollment_sample(mono, &[]).unwrap();

        assert_eq!(sample.len(), ms(60_000));
        assert_eq!(sample.last(), Some(&((ms(60_000) - 1) as f32)));
    }
}
