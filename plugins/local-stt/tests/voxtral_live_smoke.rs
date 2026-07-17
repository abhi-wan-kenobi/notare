//! Plugin-level live smoke test for the Voxtral (llama.cpp) internal server
//! wiring (issue #16 Phase B).
//!
//! This drives the actual `InternalSTTActor` + `TranscribeService<LoadedVoxtral>`
//! seam that `LocalStt::start_server` spins up for a real desktop session
//! (`plugins/local-stt/src/server/internal.rs`), not just the engine crate
//! (`crates/transcribe-voxtral-llama/tests/real_model.rs` already covers
//! that in isolation). It binds a real HTTP server on a loopback port and
//! POSTs audio to `/v1/listen`, exactly the request shape the desktop app's
//! batch/final pass sends — proving the wiring, not just the engine.
//!
//! Ignored by default (needs ~3.2GB of GGUF weights on disk: the Q4_K_M
//! text-decoder + the mmproj audio-encoder, both from
//! <https://huggingface.co/ggml-org/Voxtral-Mini-3B-2507-GGUF>). Run with:
//!
//! ```sh
//! VOXTRAL_MODEL_DIR=/path/to/dir/holding/both/gguf/files \
//!   cargo test -p tauri-plugin-local-stt --features voxtral-llama \
//!   --test voxtral_live_smoke -- --ignored --nocapture
//! ```
//!
//! `VOXTRAL_MODEL_DIR` must contain both
//! `hypr_voxtral_llama_model::VoxtralLlamaModel::Mini3bQ4KM::weight_file()`
//! and `::mmproj_file()` at their exact expected names.

#![cfg(feature = "voxtral-llama")]

use std::time::Instant;

use hypr_voxtral_llama_model::VoxtralLlamaModel;
use ractor::{Actor, call_t};
use tauri_plugin_local_stt::internal::{
    InternalModel, InternalSTTActor, InternalSTTArgs, InternalSTTMessage,
};

#[tokio::test]
#[ignore]
async fn voxtral_internal_server_transcribes_real_audio_over_http() {
    let _ = tracing_subscriber::fmt::try_init();

    let model_dir = std::env::var("VOXTRAL_MODEL_DIR")
        .expect("set VOXTRAL_MODEL_DIR to the directory holding both Voxtral GGUF files");

    // `InternalModel::model_path` resolves to `model_cache_dir/<model_dir()>`
    // (see `plugins/local-stt/src/server/internal.rs`), which mirrors
    // `LocalModel::install_path`'s `models_base/stt/<model_dir()>` layout one
    // level up (the plugin always passes `models_dir()`, i.e. the `.../stt`
    // directory, as `model_cache_dir`). Symlink the pre-downloaded weights
    // into that exact layout instead of copying ~3.2GB.
    let cache_root = tempfile::tempdir().unwrap();
    let model = VoxtralLlamaModel::Mini3bQ4KM;
    let link_target = cache_root.path().join(model.model_dir());

    #[cfg(unix)]
    std::os::unix::fs::symlink(&model_dir, &link_target).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&model_dir, &link_target).unwrap();

    for file in model.files() {
        let path = link_target.join(file.name);
        assert!(
            path.is_file(),
            "expected {path:?} in VOXTRAL_MODEL_DIR (symlinked from {model_dir})"
        );
    }

    let load_started = Instant::now();
    let (actor_ref, actor_handle) = Actor::spawn(
        Some(InternalSTTActor::name()),
        InternalSTTActor,
        InternalSTTArgs {
            model_type: InternalModel::VoxtralLlama(model),
            model_cache_dir: cache_root.path().to_path_buf(),
        },
    )
    .await
    .expect("internal STT actor failed to start");

    let info = call_t!(actor_ref, InternalSTTMessage::GetHealth, 30_000)
        .expect("health check call failed");
    let base_url = info.url.expect("internal server did not report a base_url");
    println!(
        "internal server up at {base_url} ({:.1}s)",
        load_started.elapsed().as_secs_f64()
    );

    // Same fixture + 25s VAD-chunk cap the engine-level test
    // (`crates/transcribe-voxtral-llama/tests/real_model.rs`) uses, so the
    // two results are directly comparable.
    let source =
        hypr_audio_utils::source_from_path(hypr_data::english_1::AUDIO_PART2_16000HZ_PATH)
            .unwrap();
    let samples = hypr_audio_utils::resample_audio(source, 16_000).unwrap();
    let samples = &samples[..samples.len().min(25 * 16_000)];
    let audio_seconds = samples.len() as f64 / 16_000.0;
    let wav_bytes = encode_wav_16khz_mono(samples);

    let client = reqwest::Client::new();
    let request_started = Instant::now();
    // No `?transcription_mode` / websocket upgrade here: this is the exact
    // request the desktop app's batch ("final pass") path sends — a plain
    // POST with an `audio/*` content-type. `TranscribeService::call` routes
    // it to `batch::handle_batch`, which VAD-chunks the audio and calls
    // `SttEngineSession::transcribe` per chunk — the same batch-per-utterance
    // seam `/v1/listen`'s websocket "live" path uses (see the doc comment on
    // the `InternalModel::VoxtralLlama` router arm).
    let response = client
        .post(format!("{base_url}/listen"))
        .header("content-type", "audio/wav")
        .body(wav_bytes)
        .send()
        .await
        .expect("POST /v1/listen failed");

    let status = response.status();
    let body_text = response.text().await.expect("failed to read response body");
    let elapsed = request_started.elapsed();
    assert!(
        status.is_success(),
        "expected a successful response, got {status}: {body_text}"
    );

    let body: serde_json::Value =
        serde_json::from_str(&body_text).expect("response body was not JSON");
    let transcript = body["results"]["channels"][0]["alternatives"][0]["transcript"]
        .as_str()
        .unwrap_or_default();

    println!("plugin-level transcript: {transcript}");
    println!(
        "audio: {audio_seconds:.1}s, request: {:.1}s, RTF: {:.3}",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() / audio_seconds
    );

    assert!(
        !transcript.trim().is_empty(),
        "expected a non-empty transcript from the internal server, got body: {body_text}"
    );

    actor_ref.stop(None);
    let _ = actor_handle.await;
}

/// Minimal 16-bit PCM mono WAV encoder — the batch handler decodes whatever
/// `hypr_audio_utils::source_from_path` (rodio) understands from the
/// `content-type`-derived extension, and re-wrapping already-resampled f32
/// samples as a small in-memory WAV keeps this test independent of any
/// pre-existing WAV fixture's original sample rate/channel layout.
fn encode_wav_16khz_mono(samples: &[f32]) -> Vec<u8> {
    let sample_rate: u32 = 16_000;
    let bits_per_sample: u16 = 16;
    let channels: u16 = 1;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_len = (samples.len() * 2) as u32;

    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }

    out
}
