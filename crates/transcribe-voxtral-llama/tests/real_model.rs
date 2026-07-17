//! End-to-end test against the real downloaded model (CPU).
//!
//! Ignored by default (needs ~3.2GB of GGUF weights on disk: the Q4_K_M
//! text-decoder + the mmproj audio-encoder, both from
//! <https://huggingface.co/ggml-org/Voxtral-Mini-3B-2507-GGUF>). Run with:
//!
//! ```sh
//! VOXTRAL_MODEL_DIR=/path/to/dir/holding/both/gguf/files \
//!   cargo test -p transcribe-voxtral-llama --test real_model -- --ignored --nocapture
//! ```
//!
//! `VOXTRAL_MODEL_DIR` must contain both
//! `transcribe_voxtral_llama::WEIGHT_FILE` and
//! `transcribe_voxtral_llama::MMPROJ_FILE` at their exact expected names.

use std::path::Path;

use hypr_model_manager::ModelLoader;
use hypr_transcribe_core::{SttEngine, SttEngineSession};
use transcribe_voxtral_llama::LoadedVoxtral;

#[test]
#[ignore]
fn transcribes_real_english_audio_on_cpu() {
    tracing_subscriber_init();

    let model_dir =
        std::env::var("VOXTRAL_MODEL_DIR").expect("set VOXTRAL_MODEL_DIR to the model directory");

    let source = hypr_audio_utils::source_from_path(hypr_data::english_1::AUDIO_PART2_16000HZ_PATH)
        .unwrap();
    let samples = hypr_audio_utils::resample_audio(source, 16_000).unwrap();
    // Cap at libmtmd's ~30s fixed-chunk ceiling (and Notare's own 25s VAD
    // chunk ceiling) — this is a batch engine, not a streaming one.
    let samples = &samples[..samples.len().min(25 * 16_000)];
    let audio_seconds = samples.len() as f64 / 16_000.0;

    let load_started = std::time::Instant::now();
    let engine = LoadedVoxtral::load(Path::new(&model_dir)).unwrap();
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

    for segment in &segments {
        assert!(segment.end > segment.start, "segment end must follow start");
        assert!(segment.start >= 0.0 && segment.end <= audio_seconds + 1.0);
    }
}

/// Cheap best-effort tracing setup so `--nocapture` shows load/inference
/// logs; failure to install a second global subscriber (e.g. re-running in
/// the same process) is not fatal to the test.
fn tracing_subscriber_init() {
    let _ = tracing_subscriber::fmt::try_init();
}
