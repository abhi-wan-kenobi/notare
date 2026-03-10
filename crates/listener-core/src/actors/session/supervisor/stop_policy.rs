use crate::{InMemoryRecordingDisposition, RecordingMode, StopSessionParams, TranscriptionMode};

pub(super) fn resolve_in_memory_recording_disposition(
    recording_mode: RecordingMode,
    current_transcription_mode: TranscriptionMode,
    params: &StopSessionParams,
) -> Option<InMemoryRecordingDisposition> {
    if recording_mode != RecordingMode::Memory {
        return None;
    }

    Some(
        params
            .in_memory_recording
            .unwrap_or_else(|| default_in_memory_recording_disposition(current_transcription_mode)),
    )
}

pub(super) fn default_in_memory_recording_disposition(
    current_transcription_mode: TranscriptionMode,
) -> InMemoryRecordingDisposition {
    if current_transcription_mode == TranscriptionMode::Batch {
        InMemoryRecordingDisposition::Persist
    } else {
        InMemoryRecordingDisposition::Discard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_in_memory_disposition_discards_while_current_mode_is_live() {
        assert_eq!(
            default_in_memory_recording_disposition(TranscriptionMode::Live),
            InMemoryRecordingDisposition::Discard
        );
    }

    #[test]
    fn default_in_memory_disposition_persists_after_batch_fallback() {
        assert_eq!(
            default_in_memory_recording_disposition(TranscriptionMode::Batch),
            InMemoryRecordingDisposition::Persist
        );
    }

    #[test]
    fn stop_override_wins_over_derived_disposition() {
        let params = StopSessionParams {
            in_memory_recording: Some(InMemoryRecordingDisposition::Discard),
        };

        assert_eq!(
            resolve_in_memory_recording_disposition(
                RecordingMode::Memory,
                TranscriptionMode::Batch,
                &params,
            ),
            Some(InMemoryRecordingDisposition::Discard)
        );
    }
}
