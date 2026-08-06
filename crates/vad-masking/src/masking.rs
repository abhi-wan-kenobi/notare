use crate::{StreamingVad, VadConfig};

pub struct VadMask {
    vad: Option<StreamingVad>,
    vad_cfg: VadConfig,
}

impl VadMask {
    pub fn new() -> Self {
        Self {
            vad: None,
            vad_cfg: VadConfig::default(),
        }
    }

    pub fn with_vad_config(mut self, cfg: VadConfig) -> Self {
        self.vad_cfg = cfg;
        self
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        let vad = self
            .vad
            .get_or_insert_with(|| StreamingVad::with_config(samples.len(), self.vad_cfg.clone()));

        vad.process_in_place(samples, |frame, is_speech| {
            if !is_speech {
                frame.fill(0.0);
            }
        });
    }
}

impl Default for VadMask {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // All tests below are deterministic: they only exercise the amplitude-floor
    // path of the underlying StreamingVad. Any buffer whose RMS is below
    // `amplitude_floor` is classified without ever invoking the earshot model,
    // so no model file, audio fixture, or GPU is required and the outcome does
    // not depend on the VAD's learned weights.

    #[test]
    fn empty_input_is_a_noop() {
        // An empty buffer must not panic and must not lazily construct the VAD.
        let mut mask = VadMask::new();
        let mut buf: Vec<f32> = Vec::new();
        mask.process(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn sub_floor_audio_is_masked_to_silence() {
        // start_in_speech:false + hangover:0 means the very first non-speech
        // frame is emitted as non-speech, so masking zeroes it immediately.
        // 0.001 RMS sits below the 0.01 floor -> deterministically non-speech.
        let mut mask = VadMask::new().with_vad_config(VadConfig {
            hangover_frames: 0,
            amplitude_floor: 0.01,
            start_in_speech: false,
        });
        let mut buf = vec![0.001_f32; 640];
        mask.process(&mut buf);
        assert!(
            buf.iter().all(|&s| s == 0.0),
            "every sub-floor sample should have been zeroed by the mask",
        );
    }

    #[test]
    fn hangover_preserves_a_single_sub_floor_frame() {
        // With start_in_speech:true and a non-zero hangover, the first quiet
        // frame is held as speech (the hangover tail) and therefore NOT masked.
        // 320 samples -> exactly one frame, so we observe the tail directly.
        let mut mask = VadMask::new().with_vad_config(VadConfig {
            hangover_frames: 6,
            amplitude_floor: 0.01,
            start_in_speech: true,
        });
        let mut buf = vec![0.001_f32; 320];
        mask.process(&mut buf);
        assert!(
            buf.iter().all(|&s| s == 0.001_f32),
            "a hangover-covered quiet frame must be passed through untouched",
        );
    }

    #[test]
    fn default_matches_new() {
        // Smoke-test the Default impl: it must behave like ::new() (lazily
        // constructs the VAD, no panic on first process()). 0.0001 is below
        // the default 0.0005 amplitude floor, so this stays on the
        // deterministic, model-free path.
        let mut mask = VadMask::default();
        let mut buf = vec![0.0001_f32; 320];
        mask.process(&mut buf);
        // Default VadConfig starts in speech with hangover 6, so this quiet
        // frame is preserved rather than zeroed.
        assert!(buf.iter().all(|&s| s == 0.0001_f32));
    }
}
