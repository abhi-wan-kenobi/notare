//! D3 repro probe: feed increasingly long real-speech buffers straight into
//! `ParakeetSession::transcribe` to find the duration at which the ORT
//! inference path errors, hangs, or hard-aborts the process.
//!
//! Ignored by default (needs the ~670MB model on disk). Run with:
//!
//! ```sh
//! PARAKEET_MODEL_DIR=/path/to/parakeet-tdt-0.6b-v3-int8 \
//!   cargo test -p parakeet-onnx --test probe_long_buffer -- --ignored --nocapture
//! ```
//!
//! Optional `PROBE_SECS=20` limits it to a single duration (so a hard native
//! abort at one length can be isolated in its own process without the earlier
//! lengths hiding it).

use std::io::Write;
use std::path::Path;

use hypr_model_manager::ModelLoader;
use hypr_transcribe_core::{SttEngine, SttEngineSession};
use parakeet_onnx::LoadedParakeet;

#[test]
#[ignore]
fn probe_boundary_where_parakeet_breaks_on_long_buffers() {
    let model_dir =
        std::env::var("PARAKEET_MODEL_DIR").expect("set PARAKEET_MODEL_DIR to the model directory");

    // Real continuous speech, resampled to the pipeline's 16kHz. english_1 is
    // >100s long, so every probe length below is real audio (not silence).
    let source = hypr_audio_utils::source_from_path(hypr_data::english_1::AUDIO_PATH).unwrap();
    let full = hypr_audio_utils::resample_audio(source, 16_000).unwrap();
    let total_secs = full.len() as f64 / 16_000.0;
    println!("fixture: {total_secs:.1}s of 16kHz speech available");

    let engine = LoadedParakeet::load(Path::new(&model_dir)).unwrap();
    let mut session = engine.session(vec![]).unwrap();

    let durations: Vec<usize> = match std::env::var("PROBE_SECS") {
        Ok(v) => v
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect(),
        Err(_) => vec![5, 10, 15, 18, 20, 22, 24, 25, 26, 28, 30],
    };

    for secs in durations {
        let n = (secs * 16_000).min(full.len());
        // A pauseless buffer: the exact shape the streaming path hands the
        // engine when the 20s VAD target lets an utterance grow unbroken.
        let buf = &full[..n];
        print!("probe {secs:>3}s ({n} samples): ");
        std::io::stdout().flush().ok();

        let started = std::time::Instant::now();
        let result = session.transcribe(buf);
        let elapsed = started.elapsed().as_secs_f64();

        match result {
            Ok(segments) => {
                let text_len: usize = segments.iter().map(|s| s.text.len()).sum();
                println!(
                    "OK  {} segments, {} transcript bytes, decode {:.1}s (RTF {:.2})",
                    segments.len(),
                    text_len,
                    elapsed,
                    elapsed / secs as f64
                );
            }
            Err(error) => {
                println!("ERR after {elapsed:.1}s: {error}");
            }
        }
        std::io::stdout().flush().ok();
    }

    println!("probe completed without a hard process abort");
}
