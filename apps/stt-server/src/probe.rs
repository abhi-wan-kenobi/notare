use std::path::Path;
use std::time::Instant;

use hypr_whisper_local::LoadedWhisper;

/// Runs a short transcription probe to verify GPU offload and measure performance.
///
/// It loads the model, transcribes a 1.0-second silent audio segment, and returns
/// the calculated realtime factor (audio duration / elapsed time). Returns `None`
/// if the model path is not a file or if loading/transcribing fails.
pub fn run_probe(model_path: &Path) -> Option<f32> {
    if !model_path.is_file() {
        return None;
    }

    let loaded = match LoadedWhisper::builder()
        .model_path(model_path.to_string_lossy().into_owned())
        .build()
    {
        Ok(l) => l,
        Err(error) => {
            tracing::warn!(%error, "probe: failed to load whisper model");
            return None;
        }
    };

    let mut session = match loaded.session(vec![]) {
        Ok(s) => s,
        Err(error) => {
            tracing::warn!(%error, "probe: failed to build whisper session");
            return None;
        }
    };

    // 1.0 seconds of silence (16,000 float samples at 16kHz)
    let audio_len_secs = 1.0f32;
    let samples = vec![0.0f32; 16000];

    let start = Instant::now();
    if let Err(error) = session.transcribe(&samples) {
        tracing::warn!(%error, "probe: whisper transcription failed");
        return None;
    }
    let elapsed = start.elapsed().as_secs_f32();

    if elapsed <= 0.0 {
        return Some(999.0); // Safe fallback to avoid division by zero or NaN
    }

    let factor = audio_len_secs / elapsed;
    tracing::info!(
        elapsed_secs = elapsed,
        realtime_factor = factor,
        "probe: completed successfully"
    );

    Some(factor)
}
