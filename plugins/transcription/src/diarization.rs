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

/// Run on-device speaker diarization over a recorded audio file and assign each
/// supplied word a speaker index by timestamp overlap.
///
/// Pure compute: the caller persists the results as `provider_speaker_index`
/// hints, exactly like the cloud-provider speaker path — so the transcript
/// render/label pipeline is unchanged. Runs on the bundled models (no download).
#[tauri::command]
#[specta::specta]
pub async fn run_diarization(
    audio_path: String,
    num_speakers: Option<i32>,
    words: Vec<DiarWordInput>,
) -> Result<Vec<DiarWordSpeaker>, String> {
    // Diarization + ONNX inference is blocking/CPU-heavy — keep it off the async
    // runtime's worker threads.
    tokio::task::spawn_blocking(move || diarize_and_align(audio_path, num_speakers, words))
        .await
        .map_err(|e| e.to_string())?
}

fn diarize_and_align(
    audio_path: String,
    num_speakers: Option<i32>,
    words: Vec<DiarWordInput>,
) -> Result<Vec<DiarWordSpeaker>, String> {
    use hypr_transcript::diar_align::{AlignableWord, SpeakerSpan, align_words_to_speakers};

    // Decode + resample the recording to 16 kHz mono f32 (what the diarizer wants).
    let meta = hypr_audio_utils::audio_file_metadata(&audio_path).map_err(|e| e.to_string())?;
    let source = hypr_audio_utils::source_from_path(&audio_path).map_err(|e| e.to_string())?;
    let resampled = hypr_audio_utils::resample_audio(source, 16_000).map_err(|e| e.to_string())?;
    let channels = std::num::NonZeroU8::new(meta.channels).unwrap_or(std::num::NonZeroU8::MIN);
    let mono = hypr_audio_utils::mix_down_to_mono(&resampled, channels);

    // Diarize into speaker spans, then align them onto the supplied words.
    let mut diarizer = hypr_diarization::Diarizer::from_bundled().map_err(|e| e.to_string())?;
    let segments = diarizer
        .diarize(&mono, num_speakers)
        .map_err(|e| e.to_string())?;

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

    Ok(align_words_to_speakers(&alignable, &spans)
        .into_iter()
        .map(|(word_id, speaker_index)| DiarWordSpeaker {
            word_id,
            speaker_index,
        })
        .collect())
}
