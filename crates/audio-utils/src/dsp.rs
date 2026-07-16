//! Streaming DSP helpers for the capture / transcription paths.
//!
//! [`PeakLimiter`] and [`MonoMixdown`] replace the naive sum-and-hard-clamp
//! mixdown (`mix_audio_f32`) at stateful call sites: hard clamping flat-tops
//! the waveform whenever mic and speaker overlap at high level, which is
//! audible as harsh "radio break" distortion and degrades STT accuracy.
//!
//! [`LoudnessNormalizer`] levels a transcription-bound stream to a target
//! integrated loudness (EBU R128, -23 LUFS) so quiet microphones stay
//! intelligible next to loud system audio.
//!
//! The headroom-aware mixdown and mic loudness normalization approach is
//! adapted from Meetily (MIT, <https://github.com/Zackriya-Solutions/meeting-minutes>,
//! `audio/audio_processing.rs` / `audio/pipeline.rs`), reimplemented with
//! smoothed gain to avoid the per-sample hard clipping and stepped gain
//! changes of the original.

use ebur128::{EbuR128, Mode};

const LIMITER_DEFAULT_CEILING: f32 = 0.98;
const LIMITER_DEFAULT_RELEASE_MS: f32 = 150.0;

const NORMALIZER_TARGET_LUFS: f64 = -23.0;
const NORMALIZER_MAX_GAIN: f32 = 8.0; // +18 dB: never boost a noise floor further
const NORMALIZER_MIN_GAIN: f32 = 0.125; // -18 dB
const NORMALIZER_ANALYSIS_CHUNK: usize = 512;
const NORMALIZER_GAIN_SMOOTHING_MS: f32 = 400.0;

/// Zero-latency peak limiter: instant attack, exponential release.
///
/// Guarantees `|output| <= ceiling` for every sample while staying fully
/// transparent (unity gain) for signals that never exceed the ceiling.
/// Unlike a hard clamp, an overshoot lowers the gain smoothly for the
/// following samples instead of flat-topping each one independently.
pub struct PeakLimiter {
    ceiling: f32,
    release_coeff: f32,
    gain: f32,
}

impl PeakLimiter {
    pub fn new(sample_rate: u32) -> Self {
        Self::with_params(
            sample_rate,
            LIMITER_DEFAULT_CEILING,
            LIMITER_DEFAULT_RELEASE_MS,
        )
    }

    pub fn with_params(sample_rate: u32, ceiling: f32, release_ms: f32) -> Self {
        let release_samples = (sample_rate as f32 * release_ms / 1000.0).max(1.0);
        Self {
            ceiling: ceiling.clamp(f32::EPSILON, 1.0),
            release_coeff: 1.0 - (-1.0 / release_samples).exp(),
            gain: 1.0,
        }
    }

    pub fn process_sample(&mut self, sample: f32) -> f32 {
        let sample = if sample.is_finite() { sample } else { 0.0 };
        let amplitude = sample.abs();
        let allowed = if amplitude > self.ceiling {
            self.ceiling / amplitude
        } else {
            1.0
        };

        // Release toward unity, but attack (reduction) is instantaneous so the
        // output can never overshoot the ceiling.
        self.gain = (self.gain + (1.0 - self.gain) * self.release_coeff).min(allowed);
        sample * self.gain
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for sample in samples {
            *sample = self.process_sample(*sample);
        }
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }
}

/// Clip-safe two-stream mono mixdown.
///
/// Sums the streams at unity gain (so a lone talker is untouched) and runs
/// the sum through a [`PeakLimiter`] so simultaneous mic + speaker activity
/// ducks gracefully instead of hard clipping. Streams of different lengths
/// are zero-padded to the longer one, matching `mix_audio_f32` semantics.
pub struct MonoMixdown {
    limiter: PeakLimiter,
}

impl MonoMixdown {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            limiter: PeakLimiter::new(sample_rate),
        }
    }

    pub fn mix(&mut self, first: &[f32], second: &[f32]) -> Vec<f32> {
        let frames = first.len().max(second.len());
        let mut output = Vec::with_capacity(frames);
        for index in 0..frames {
            let a = first.get(index).copied().unwrap_or(0.0);
            let b = second.get(index).copied().unwrap_or(0.0);
            output.push(self.limiter.process_sample(a + b));
        }
        output
    }
}

/// Streaming EBU R128 loudness normalizer for a mono stream.
///
/// Tracks integrated loudness (gated per BS.1770, so silence does not drag
/// the estimate down) and applies a bounded, smoothly ramped gain toward the
/// -23 LUFS broadcast target, followed by a [`PeakLimiter`] safety stage.
///
/// Intended for the transcription-bound copy of the microphone stream; the
/// recorded session audio must not pass through this.
pub struct LoudnessNormalizer {
    analyzer: Option<EbuR128>,
    analysis_buffer: Vec<f32>,
    target_gain: f32,
    smoothed_gain: f32,
    smoothing_coeff: f32,
    limiter: PeakLimiter,
}

impl LoudnessNormalizer {
    pub fn new(sample_rate: u32) -> Self {
        let analyzer = EbuR128::new(1, sample_rate, Mode::I)
            .map_err(
                |error| tracing::warn!(error.message = %error, "ebur128_init_failed_passthrough"),
            )
            .ok();

        let smoothing_samples =
            (sample_rate as f32 * NORMALIZER_GAIN_SMOOTHING_MS / 1000.0).max(1.0);

        Self {
            analyzer,
            analysis_buffer: Vec::with_capacity(NORMALIZER_ANALYSIS_CHUNK),
            target_gain: 1.0,
            smoothed_gain: 1.0,
            smoothing_coeff: 1.0 - (-1.0 / smoothing_samples).exp(),
            limiter: PeakLimiter::new(sample_rate),
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut output = Vec::with_capacity(samples.len());

        for &sample in samples {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            self.observe(sample);

            self.smoothed_gain += (self.target_gain - self.smoothed_gain) * self.smoothing_coeff;
            output.push(self.limiter.process_sample(sample * self.smoothed_gain));
        }

        output
    }

    fn observe(&mut self, sample: f32) {
        let Some(analyzer) = self.analyzer.as_mut() else {
            return;
        };

        self.analysis_buffer.push(sample);
        if self.analysis_buffer.len() < NORMALIZER_ANALYSIS_CHUNK {
            return;
        }

        if let Err(error) = analyzer.add_frames_f32(&self.analysis_buffer) {
            tracing::warn!(error.message = %error, "ebur128_add_frames_failed");
        } else if let Ok(current_lufs) = analyzer.loudness_global()
            && current_lufs.is_finite()
            && current_lufs < 0.0
        {
            let gain_db = NORMALIZER_TARGET_LUFS - current_lufs;
            let gain = 10_f32.powf(gain_db as f32 / 20.0);
            self.target_gain = gain.clamp(NORMALIZER_MIN_GAIN, NORMALIZER_MAX_GAIN);
        }

        self.analysis_buffer.clear();
    }

    pub fn current_gain(&self) -> f32 {
        self.smoothed_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 16_000;

    fn sine(amplitude: f32, frequency_hz: f32, seconds: f32) -> Vec<f32> {
        let total = (SAMPLE_RATE as f32 * seconds) as usize;
        (0..total)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * frequency_hz * i as f32 / SAMPLE_RATE as f32)
                        .sin()
            })
            .collect()
    }

    #[test]
    fn limiter_is_transparent_for_quiet_signals() {
        let mut limiter = PeakLimiter::new(SAMPLE_RATE);
        let input = sine(0.3, 440.0, 0.1);
        let output: Vec<f32> = input.iter().map(|&s| limiter.process_sample(s)).collect();
        assert_eq!(input, output);
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let mut limiter = PeakLimiter::new(SAMPLE_RATE);
        for &sample in sine(1.9, 300.0, 0.5).iter() {
            let out = limiter.process_sample(sample);
            assert!(out.abs() <= LIMITER_DEFAULT_CEILING + 1e-6, "out = {out}");
        }
    }

    #[test]
    fn limiter_recovers_gain_after_peak() {
        let mut limiter = PeakLimiter::new(SAMPLE_RATE);
        let peaked = limiter.process_sample(2.0);
        assert!((peaked.abs() - LIMITER_DEFAULT_CEILING).abs() < 1e-6);
        assert!(limiter.gain() < 0.5);

        // One second of quiet signal is far beyond the 150 ms release window.
        let mut last = 0.0;
        for &sample in sine(0.1, 440.0, 1.0).iter() {
            last = limiter.process_sample(sample).abs().max(last * 0.999);
        }
        assert!(limiter.gain() > 0.99, "gain = {}", limiter.gain());
        let _ = last;
    }

    #[test]
    fn limiter_zeroes_non_finite_samples() {
        let mut limiter = PeakLimiter::new(SAMPLE_RATE);
        assert_eq!(limiter.process_sample(f32::NAN), 0.0);
        assert_eq!(limiter.process_sample(f32::INFINITY), 0.0);
        assert_eq!(limiter.gain(), 1.0);
    }

    #[test]
    fn mixdown_passes_single_active_stream_through() {
        let mut mixdown = MonoMixdown::new(SAMPLE_RATE);
        let mic = sine(0.5, 200.0, 0.05);
        let silence = vec![0.0; mic.len()];
        let mixed = mixdown.mix(&mic, &silence);
        assert_eq!(mixed, mic);
    }

    #[test]
    fn mixdown_stays_below_ceiling_on_full_scale_overlap() {
        let mut mixdown = MonoMixdown::new(SAMPLE_RATE);
        let mic = sine(0.95, 200.0, 0.2);
        let speaker = sine(0.95, 210.0, 0.2);
        let mixed = mixdown.mix(&mic, &speaker);
        assert!(
            mixed
                .iter()
                .all(|s| s.abs() <= LIMITER_DEFAULT_CEILING + 1e-6)
        );
    }

    #[test]
    fn mixdown_zero_pads_length_mismatch() {
        let mut mixdown = MonoMixdown::new(SAMPLE_RATE);
        let mixed = mixdown.mix(&[0.1, 0.2], &[0.3]);
        assert_eq!(mixed.len(), 2);
        assert!((mixed[0] - 0.4).abs() < 1e-6);
        assert!((mixed[1] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn mixdown_avoids_hard_clip_flat_tops() {
        // The legacy clamp mix produces long runs of samples pinned to
        // exactly +/-1.0; the limiter must not.
        let mic = sine(0.9, 200.0, 0.2);
        let speaker = sine(0.9, 200.0, 0.2); // in-phase worst case

        let legacy = crate::mix_audio_f32(&mic, &speaker);
        let clipped = legacy.iter().filter(|s| s.abs() >= 1.0).count();
        assert!(clipped > 100, "expected legacy clamp to clip, clipped = {clipped}");

        let mut mixdown = MonoMixdown::new(SAMPLE_RATE);
        let mixed = mixdown.mix(&mic, &speaker);
        let pinned = mixed
            .iter()
            .filter(|s| s.abs() >= LIMITER_DEFAULT_CEILING - 1e-6)
            .count();
        assert!(
            pinned < clipped / 4,
            "limiter should not flat-top: pinned = {pinned}, legacy clipped = {clipped}"
        );
    }

    #[test]
    fn normalizer_boosts_quiet_signal() {
        let mut normalizer = LoudnessNormalizer::new(SAMPLE_RATE);
        let input = sine(0.01, 400.0, 4.0);
        let output = normalizer.process(&input);

        let tail = &output[output.len() - SAMPLE_RATE as usize..];
        let tail_peak = tail.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(tail_peak > 0.04, "tail_peak = {tail_peak}");
        assert!(normalizer.current_gain() > 3.0);
        assert!(normalizer.current_gain() <= NORMALIZER_MAX_GAIN);
    }

    #[test]
    fn normalizer_attenuates_loud_signal() {
        let mut normalizer = LoudnessNormalizer::new(SAMPLE_RATE);
        let input = sine(0.9, 400.0, 4.0);
        let output = normalizer.process(&input);

        let tail = &output[output.len() - SAMPLE_RATE as usize..];
        let tail_peak = tail.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(tail_peak < 0.5, "tail_peak = {tail_peak}");
        assert!(normalizer.current_gain() < 1.0);
        assert!(normalizer.current_gain() >= NORMALIZER_MIN_GAIN);
    }

    #[test]
    fn normalizer_leaves_silence_untouched() {
        let mut normalizer = LoudnessNormalizer::new(SAMPLE_RATE);
        let input = vec![0.0f32; SAMPLE_RATE as usize];
        let output = normalizer.process(&input);
        assert_eq!(output, input);
        assert!((normalizer.current_gain() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn normalizer_output_respects_ceiling() {
        let mut normalizer = LoudnessNormalizer::new(SAMPLE_RATE);
        // Quiet lead-in raises the gain, then a sudden full-scale burst must
        // still be limited.
        let mut input = sine(0.02, 400.0, 3.0);
        input.extend(sine(1.0, 400.0, 0.5));
        let output = normalizer.process(&input);
        assert!(
            output
                .iter()
                .all(|s| s.abs() <= LIMITER_DEFAULT_CEILING + 1e-6)
        );
    }
}
