//! Calibration harness for auto speaker-count detection.
//!
//! Sweeps the sherpa `FastClustering` `threshold` (and `min_duration_on/off`)
//! against the labelled fixtures in `hypr-data`, printing the detected speaker
//! count vs. ground truth so we can pick a default that auto-detects the right
//! number of speakers. Run: `cargo run -p diarization --example threshold_sweep --release`.
//!
//! Lower threshold -> more clusters (over-splits); higher -> fewer (merges).
//! Upstream sherpa-onnx's own CLI example uses 0.90 for auto-K, so the struct
//! default of 0.5 is expected to over-count here.

use std::collections::BTreeSet;

use sherpa_rs::diarize::{Diarize, DiarizeConfig};

fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

fn count_speakers(
    seg: &std::path::Path,
    emb: &std::path::Path,
    samples: &[f32],
    threshold: f32,
    min_on: f32,
    min_off: f32,
) -> usize {
    let config = DiarizeConfig {
        num_clusters: Some(-1),
        threshold: Some(threshold),
        min_duration_on: Some(min_on),
        min_duration_off: Some(min_off),
        ..Default::default()
    };
    let mut d = Diarize::new(seg, emb, config).expect("diarize new");
    let segments = d.compute(samples.to_vec(), None).expect("compute");
    segments
        .iter()
        .map(|s| s.speaker)
        .collect::<BTreeSet<_>>()
        .len()
}

fn main() {
    let (seg, emb) = diarization::bundled_model_paths().expect("bundled models");

    // (name, pcm16 audio, ground-truth speaker count)
    let clips: [(&str, &[u8], usize); 4] = [
        ("english_2", hypr_data::english_2::AUDIO, 2),
        ("english_1", hypr_data::english_1::AUDIO, 2),
        ("korean_1", hypr_data::korean_1::AUDIO, 2),
        ("korean_2", hypr_data::korean_2::AUDIO, 3),
    ];

    let thresholds = [0.90f32, 0.93, 0.95, 0.97, 0.99];
    // min_duration made no difference in the coarse sweep, so fix at upstream's.
    let min_durations = [(0.3f32, 0.5f32)];

    let decoded: Vec<(&str, Vec<f32>, usize)> = clips
        .iter()
        .map(|(name, audio, truth)| (*name, pcm16_to_f32(audio), *truth))
        .collect();

    for (min_on, min_off) in min_durations {
        println!("\n=== min_duration_on={min_on}  min_duration_off={min_off} ===");
        print!("{:<12}", "clip \\ thr");
        for t in thresholds {
            print!("{t:>6}");
        }
        println!("  truth");
        for (name, samples, truth) in &decoded {
            print!("{name:<12}");
            for &t in &thresholds {
                let n = count_speakers(&seg, &emb, samples, t, min_on, min_off);
                print!("{n:>6}");
            }
            println!("  ({truth})");
        }
    }
}
