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
    tokio::task::spawn_blocking(move || diarize_and_align(audio_path, num_speakers, words, enrolled))
        .await
        .map_err(|e| e.to_string())?
}

/// Compute a speaker embedding for a short enrollment clip. The caller stores it
/// as a voice profile (`voice_profiles` table) against a human. Uses the same
/// bundled embedding model as diarization, so enrolled profiles are directly
/// comparable to diarized speakers.
#[tauri::command]
#[specta::specta]
pub async fn compute_voice_embedding(audio_path: String) -> Result<Vec<f32>, String> {
    tokio::task::spawn_blocking(move || {
        let mono = decode_to_mono_16k(&audio_path)?;
        let mut index = hypr_diarization::VoiceProfileIndex::from_bundled().map_err(stringify)?;
        // Enroll under a throwaway id, then read the stored embedding back.
        index
            .enroll("__probe__".to_string(), &mono)
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

    let word_speakers = align_words_to_speakers(&alignable, &spans)
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
    let mut index =
        hypr_diarization::VoiceProfileIndex::from_profiles(embedding, profiles).map_err(stringify)?;

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
        let start = (seg.start_ms as usize) * SAMPLE_RATE_HZ / 1000;
        let end = ((seg.end_ms as usize) * SAMPLE_RATE_HZ / 1000).min(mono.len());
        if start >= end {
            continue;
        }
        if let Some(human_id) = index.identify(&mono[start..end], MATCH_THRESHOLD).map_err(stringify)? {
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
