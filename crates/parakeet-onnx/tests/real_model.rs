//! End-to-end test against the real downloaded model.
//!
//! Ignored by default (needs ~670MB of model files on disk). Run with:
//!
//! ```sh
//! PARAKEET_MODEL_DIR=/path/to/parakeet-tdt-0.6b-v3-int8 \
//!   cargo test -p parakeet-onnx --test real_model -- --ignored --nocapture
//! ```

use std::path::Path;

use hypr_model_manager::ModelLoader;
use hypr_transcribe_core::{SttEngine, SttEngineSession};
use parakeet_onnx::LoadedParakeet;

#[test]
#[ignore]
fn transcribes_real_english_audio() {
    let model_dir =
        std::env::var("PARAKEET_MODEL_DIR").expect("set PARAKEET_MODEL_DIR to the model directory");

    let source = hypr_audio_utils::source_from_path(hypr_data::english_1::AUDIO_PATH).unwrap();
    let samples = hypr_audio_utils::resample_audio(source, 16_000).unwrap();
    // First 30 seconds is plenty to prove the engine end to end and mirrors
    // the production chunk ceiling (25s).
    let samples = &samples[..samples.len().min(30 * 16_000)];
    let audio_seconds = samples.len() as f64 / 16_000.0;

    let load_started = std::time::Instant::now();
    let engine = LoadedParakeet::load(Path::new(&model_dir)).unwrap();
    println!("model load: {:.1}s", load_started.elapsed().as_secs_f64());

    let mut session = engine.session(vec![]).unwrap();

    let started = std::time::Instant::now();
    let segments = session.transcribe(samples).unwrap();
    let elapsed = started.elapsed().as_secs_f64();

    let transcript = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    println!("transcript: {transcript}");
    println!(
        "audio: {audio_seconds:.1}s, decode: {elapsed:.1}s, RTF: {:.3}",
        elapsed / audio_seconds
    );

    assert!(
        !transcript.trim().is_empty(),
        "expected a non-empty transcript"
    );
    assert!(
        segments.len() > 5,
        "expected word-level segments, got {}",
        segments.len()
    );

    let mut previous_start = f64::MIN;
    for segment in &segments {
        assert!(
            segment.start >= previous_start,
            "word starts must be monotonic"
        );
        assert!(segment.end > segment.start, "word end must follow start");
        assert!(segment.start >= 0.0 && segment.end <= audio_seconds + 1.0);
        previous_start = segment.start;
    }
}
