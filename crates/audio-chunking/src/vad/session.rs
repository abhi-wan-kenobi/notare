use std::time::Duration;

use hypr_onnx::ndarray::ArrayView1;
use hypr_vad::silero_onnx::{CHUNK_SIZE_16KHZ, SileroVad};

const SAMPLE_RATE: usize = 16000;

#[derive(Debug, Clone)]
pub struct VadChunkerConfig {
    pub positive_speech_threshold: f32,
    pub negative_speech_threshold: f32,
    pub redemption_time: Duration,
    pub pre_speech_pad: Duration,
    pub min_speech_time: Duration,
    pub min_chunk_duration: Duration,
    pub target_chunk_duration: Duration,
    pub max_negative_threshold: f32,
    /// Hard ceiling on a single speech chunk's wall-clock span. When set, an
    /// unbroken utterance is force-cut once it reaches this length even if no
    /// pause ever redeems it — the meeting/`speech` profile leaves this `None`
    /// (chunks grow to a natural pause), the `dictation` profile sets it so a
    /// continuous dictation never accumulates a giant buffer (D3: an
    /// ~20s pauseless buffer was what reached the engine and killed the WS).
    pub max_chunk_duration: Option<Duration>,
}

impl Default for VadChunkerConfig {
    fn default() -> Self {
        Self {
            positive_speech_threshold: 0.5,
            negative_speech_threshold: 0.35,
            redemption_time: Duration::from_millis(600),
            pre_speech_pad: Duration::from_millis(600),
            min_speech_time: Duration::from_millis(90),
            min_chunk_duration: Duration::from_secs(3),
            target_chunk_duration: Duration::from_secs(20),
            max_negative_threshold: 0.80,
            max_chunk_duration: None,
        }
    }
}

impl VadChunkerConfig {
    pub fn speech(redemption_time: Duration) -> Self {
        Self {
            redemption_time,
            pre_speech_pad: redemption_time,
            min_speech_time: Duration::from_millis(150),
            ..Default::default()
        }
    }

    /// Live single-speaker dictation. Unlike `speech` (tuned to grow chunks
    /// toward a ~20s meeting target), dictation redeems promptly and force-cuts
    /// a pauseless utterance at `target_chunk_duration`, so transcript arrives
    /// incrementally and no oversized buffer ever reaches the engine. The
    /// lower `max_negative_threshold` ramp cap keeps ordinary between-phrase
    /// pauses redeeming instead of resisting the cut the way the meeting ramp
    /// deliberately does.
    pub fn dictation(redemption_time: Duration) -> Self {
        let target = Duration::from_secs(6);
        Self {
            redemption_time,
            pre_speech_pad: redemption_time,
            min_speech_time: Duration::from_millis(150),
            target_chunk_duration: target,
            max_negative_threshold: 0.55,
            max_chunk_duration: Some(target),
            ..Default::default()
        }
    }

    pub fn validate(&self) -> Result<(), crate::Error> {
        validate_threshold("positive_speech_threshold", self.positive_speech_threshold)?;
        validate_threshold("negative_speech_threshold", self.negative_speech_threshold)?;
        validate_threshold("max_negative_threshold", self.max_negative_threshold)?;

        if self.negative_speech_threshold > self.positive_speech_threshold {
            return Err(crate::Error::InvalidConfig(
                "negative_speech_threshold must be <= positive_speech_threshold".into(),
            ));
        }

        if self.max_negative_threshold < self.negative_speech_threshold {
            return Err(crate::Error::InvalidConfig(
                "max_negative_threshold must be >= negative_speech_threshold".into(),
            ));
        }

        if self.redemption_time.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "redemption_time must be greater than zero".into(),
            ));
        }

        if self.min_speech_time.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "min_speech_time must be greater than zero".into(),
            ));
        }

        if self.min_chunk_duration.is_zero() {
            return Err(crate::Error::InvalidConfig(
                "min_chunk_duration must be greater than zero".into(),
            ));
        }

        if self.target_chunk_duration <= self.min_chunk_duration {
            return Err(crate::Error::InvalidConfig(
                "target_chunk_duration must be greater than min_chunk_duration".into(),
            ));
        }

        if let Some(max_chunk) = self.max_chunk_duration {
            if max_chunk < self.min_chunk_duration {
                return Err(crate::Error::InvalidConfig(
                    "max_chunk_duration must be >= min_chunk_duration".into(),
                ));
            }
        }

        Ok(())
    }
}

fn validate_threshold(name: &str, value: f32) -> Result<(), crate::Error> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(crate::Error::InvalidConfig(format!(
            "{name} must be between 0.0 and 1.0"
        )))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum VadTransition {
    SpeechStart {
        sample_start: usize,
    },
    SpeechEnd {
        detected_speech_samples: usize,
        sample_start: usize,
        sample_end: usize,
        samples: Vec<f32>,
    },
}

#[derive(Clone, Copy)]
enum VadState {
    Silence,
    Speech {
        start_sample: usize,
        confirmed: bool,
        speech_samples: usize,
    },
}

pub struct VadSession {
    silero: SileroVad,
    config: VadChunkerConfig,
    state: VadState,
    retained_audio: Vec<f32>,
    retained_start_sample: usize,
    cursor_sample: usize,
    silent_samples: usize,
    last_prob: f32,
}

impl VadSession {
    pub fn new(config: VadChunkerConfig) -> Result<Self, crate::Error> {
        config.validate()?;

        let silero = SileroVad::new_embedded()
            .map_err(|e| crate::Error::SessionCreationFailed(e.to_string()))?;
        Ok(Self {
            silero,
            config,
            state: VadState::Silence,
            retained_audio: Vec::new(),
            retained_start_sample: 0,
            cursor_sample: 0,
            silent_samples: 0,
            last_prob: 0.0,
        })
    }

    fn duration_to_samples(duration: Duration) -> usize {
        ((duration.as_millis() * SAMPLE_RATE as u128) / 1000) as usize
    }

    fn session_end_sample(&self) -> usize {
        self.retained_start_sample + self.retained_audio.len()
    }

    fn absolute_to_index(&self, sample: usize) -> usize {
        debug_assert!(sample >= self.retained_start_sample);
        debug_assert!(sample <= self.session_end_sample());
        sample - self.retained_start_sample
    }

    fn speech_end_transition(
        &self,
        start_sample: usize,
        end_sample: usize,
        detected_speech_samples: usize,
    ) -> VadTransition {
        let start_idx = self.absolute_to_index(start_sample);
        let end_idx = self.absolute_to_index(end_sample);

        VadTransition::SpeechEnd {
            detected_speech_samples,
            sample_start: start_sample,
            sample_end: end_sample,
            samples: self.retained_audio[start_idx..end_idx].to_vec(),
        }
    }

    fn reset_to_silence(&mut self) {
        self.state = VadState::Silence;
        self.silent_samples = 0;
    }

    fn trim_buffer(&mut self) {
        let min_keep_sample = match self.state {
            VadState::Silence => self
                .session_end_sample()
                .saturating_sub(Self::duration_to_samples(self.config.pre_speech_pad)),
            VadState::Speech { start_sample, .. } => start_sample,
        };
        let keep_from = min_keep_sample.min(self.cursor_sample);

        if keep_from <= self.retained_start_sample {
            return;
        }

        let drop_count = keep_from - self.retained_start_sample;
        self.retained_audio.drain(..drop_count);
        self.retained_start_sample = keep_from;
    }

    pub(crate) fn process(
        &mut self,
        audio_frame: &[f32],
    ) -> Result<Vec<VadTransition>, crate::Error> {
        self.retained_audio.extend_from_slice(audio_frame);

        let mut transitions = Vec::new();

        while self.session_end_sample().saturating_sub(self.cursor_sample) >= CHUNK_SIZE_16KHZ {
            let chunk_start = self.absolute_to_index(self.cursor_sample);
            let chunk =
                ArrayView1::from(&self.retained_audio[chunk_start..chunk_start + CHUNK_SIZE_16KHZ]);

            let prob = self
                .silero
                .process_chunk(&chunk, 16000)
                .map_err(|e| crate::Error::ProcessingFailed(e.to_string()))?;
            self.last_prob = prob;
            self.cursor_sample += CHUNK_SIZE_16KHZ;

            if let Some(t) = self.advance(prob) {
                transitions.push(t);
            }
        }

        self.trim_buffer();
        Ok(transitions)
    }

    pub(crate) fn finish(
        &mut self,
        trailing_audio: &[f32],
    ) -> Result<Vec<VadTransition>, crate::Error> {
        self.retained_audio.extend_from_slice(trailing_audio);

        let mut transitions = Vec::new();
        if let VadState::Speech {
            start_sample,
            confirmed: true,
            speech_samples,
        } = self.state
        {
            let end_sample = self.session_end_sample();
            transitions.push(self.speech_end_transition(start_sample, end_sample, speech_samples));
        }

        self.reset_to_silence();
        self.trim_buffer();
        Ok(transitions)
    }

    fn neg_threshold_for_speech_samples(&self, speech_samples: usize) -> f32 {
        let speech_secs = (speech_samples as f64) / SAMPLE_RATE as f64;
        let min_secs = self.config.min_chunk_duration.as_secs_f64();
        let target_secs = self.config.target_chunk_duration.as_secs_f64();
        let max_thresh = self.config.max_negative_threshold;
        let base_thresh = self.config.negative_speech_threshold;

        if speech_secs < min_secs {
            max_thresh
        } else if speech_secs >= target_secs {
            base_thresh
        } else {
            let t = (speech_secs - min_secs) / (target_secs - min_secs);
            max_thresh - t as f32 * (max_thresh - base_thresh)
        }
    }

    fn advance(&mut self, prob: f32) -> Option<VadTransition> {
        match self.state {
            VadState::Silence => {
                if prob > self.config.positive_speech_threshold {
                    let pad_samples = Self::duration_to_samples(self.config.pre_speech_pad);
                    let start_sample = self.cursor_sample.saturating_sub(pad_samples);
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed: false,
                        speech_samples: CHUNK_SIZE_16KHZ,
                    };
                    self.silent_samples = 0;
                }
                None
            }
            VadState::Speech {
                start_sample,
                confirmed,
                speech_samples,
            } => {
                let speech_samples = speech_samples + CHUNK_SIZE_16KHZ;

                let neg_thresh = self.neg_threshold_for_speech_samples(speech_samples);
                if prob < neg_thresh {
                    self.silent_samples += CHUNK_SIZE_16KHZ;
                } else {
                    self.silent_samples = 0;
                }

                let min_speech_samples = Self::duration_to_samples(self.config.min_speech_time);
                let redemption_samples = Self::duration_to_samples(self.config.redemption_time);

                if !confirmed && speech_samples >= min_speech_samples {
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed: true,
                        speech_samples,
                    };
                    return Some(VadTransition::SpeechStart {
                        sample_start: start_sample,
                    });
                }

                if confirmed && self.silent_samples >= redemption_samples {
                    let speech_end_sample = self.cursor_sample.saturating_sub(self.silent_samples);
                    let transition = self.speech_end_transition(
                        start_sample,
                        speech_end_sample,
                        speech_samples.saturating_sub(self.silent_samples),
                    );
                    self.reset_to_silence();
                    self.trim_buffer();
                    return Some(transition);
                }

                // Force-cut a pauseless utterance at max_chunk_duration
                // (dictation profile only). Unlike redemption this cuts at the
                // live cursor with no trailing silence, then continues the same
                // utterance in a fresh confirmed chunk starting at the cut point
                // so no audio is dropped and no spurious SpeechStart is emitted.
                if confirmed {
                    if let Some(max_chunk_samples) = self
                        .config
                        .max_chunk_duration
                        .map(Self::duration_to_samples)
                    {
                        let span = self.cursor_sample.saturating_sub(start_sample);
                        if span >= max_chunk_samples {
                            let cut_sample = self.cursor_sample;
                            let transition = self.speech_end_transition(
                                start_sample,
                                cut_sample,
                                speech_samples,
                            );
                            self.state = VadState::Speech {
                                start_sample: cut_sample,
                                confirmed: true,
                                speech_samples: 0,
                            };
                            self.silent_samples = 0;
                            self.trim_buffer();
                            return Some(transition);
                        }
                    }
                }

                if !confirmed && self.silent_samples >= redemption_samples {
                    self.reset_to_silence();
                    self.trim_buffer();
                } else {
                    self.state = VadState::Speech {
                        start_sample,
                        confirmed,
                        speech_samples,
                    };
                }

                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::*;

    fn decode_audio() -> Vec<f32> {
        rodio::Decoder::new(BufReader::new(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        ))
        .unwrap()
        .collect()
    }

    #[test]
    fn test_invalid_config_rejected() {
        let config = VadChunkerConfig {
            target_chunk_duration: Duration::from_secs(3),
            min_chunk_duration: Duration::from_secs(3),
            ..Default::default()
        };

        assert!(matches!(
            VadSession::new(config),
            Err(crate::Error::InvalidConfig(_))
        ));
    }

    #[test]
    fn test_finish_emits_confirmed_speech_with_partial_tail() {
        let audio = decode_audio();
        let mut session = VadSession::new(VadChunkerConfig::default()).unwrap();
        let mut processed = 0usize;

        while processed + CHUNK_SIZE_16KHZ + 100 <= audio.len() {
            let chunk = &audio[processed..processed + CHUNK_SIZE_16KHZ];
            let transitions = session.process(chunk).unwrap();
            processed += CHUNK_SIZE_16KHZ;

            if transitions
                .iter()
                .any(|transition| matches!(transition, VadTransition::SpeechStart { .. }))
            {
                let tail = audio[processed..processed + 100].to_vec();
                let transitions = session.finish(&tail).unwrap();

                assert_eq!(transitions.len(), 1);
                let VadTransition::SpeechEnd {
                    detected_speech_samples,
                    sample_start,
                    sample_end,
                    samples,
                } = &transitions[0]
                else {
                    panic!("expected speech end transition");
                };

                assert!(*detected_speech_samples >= CHUNK_SIZE_16KHZ);
                assert!(*sample_end > *sample_start);
                assert_eq!(&samples[samples.len() - tail.len()..], tail.as_slice());

                return;
            }
        }

        panic!("did not observe speech start in fixture audio");
    }

    #[test]
    fn test_detected_speech_samples_excludes_redeemed_trailing_silence() {
        let mut session = VadSession::new(VadChunkerConfig {
            redemption_time: Duration::from_millis(32),
            pre_speech_pad: Duration::ZERO,
            min_speech_time: Duration::from_millis(32),
            ..Default::default()
        })
        .unwrap();

        session.retained_audio = vec![1.0; CHUNK_SIZE_16KHZ * 3];
        session.cursor_sample = CHUNK_SIZE_16KHZ * 3;
        session.state = VadState::Speech {
            start_sample: 0,
            confirmed: true,
            speech_samples: CHUNK_SIZE_16KHZ * 2,
        };

        let transition = session.advance(0.0).unwrap();
        let VadTransition::SpeechEnd {
            detected_speech_samples,
            samples,
            ..
        } = transition
        else {
            panic!("expected speech end transition");
        };

        assert_eq!(detected_speech_samples, CHUNK_SIZE_16KHZ * 2);
        assert_eq!(samples.len(), CHUNK_SIZE_16KHZ * 2);
    }

    #[test]
    fn test_retained_buffer_is_bounded_for_long_silence() {
        let mut session = VadSession::new(VadChunkerConfig::default()).unwrap();
        let silence = vec![0.0; CHUNK_SIZE_16KHZ];

        for _ in 0..5000 {
            session.process(&silence).unwrap();
        }

        let max_expected =
            VadSession::duration_to_samples(session.config.pre_speech_pad) + CHUNK_SIZE_16KHZ;
        assert!(session.retained_audio.len() <= max_expected);
    }

    /// Drive `advance` with a synthetic per-frame speech probability, mirroring
    /// `process`'s cursor bookkeeping (advance one 16kHz VAD frame at a time)
    /// without invoking the real Silero model. Returns every transition seen.
    fn drive_synthetic(session: &mut VadSession, prob: f32, frames: usize) -> Vec<VadTransition> {
        let mut transitions = Vec::new();
        for _ in 0..frames {
            session.cursor_sample += CHUNK_SIZE_16KHZ;
            if let Some(t) = session.advance(prob) {
                transitions.push(t);
            }
        }
        transitions
    }

    fn speech_end_spans(transitions: &[VadTransition]) -> Vec<(usize, usize)> {
        transitions
            .iter()
            .filter_map(|t| match t {
                VadTransition::SpeechEnd {
                    sample_start,
                    sample_end,
                    ..
                } => Some((*sample_start, *sample_end)),
                _ => None,
            })
            .collect()
    }

    /// D3: the dictation profile must force-cut a pauseless utterance at its
    /// `target_chunk_duration` (6s) so no oversized buffer is ever emitted,
    /// and must keep cutting a continuously-speaking user every ~6s.
    #[test]
    fn dictation_profile_force_cuts_pauseless_speech_near_target() {
        let mut session =
            VadSession::new(VadChunkerConfig::dictation(Duration::from_millis(400))).unwrap();
        // 30s of non-silent buffer so the emitted chunk slices are in range.
        session.retained_audio = vec![1.0; SAMPLE_RATE * 30];

        // ~13s of continuous confident speech, never dropping below any
        // negative threshold, so ONLY the force-cut can end a chunk.
        let frames = (13 * SAMPLE_RATE) / CHUNK_SIZE_16KHZ;
        let spans = speech_end_spans(&drive_synthetic(&mut session, 0.95, frames));

        assert!(
            spans.len() >= 2,
            "expected at least two force-cuts in 13s of continuous speech, got {}",
            spans.len()
        );
        let max_samples = VadSession::duration_to_samples(Duration::from_secs(6));
        for (start, end) in &spans {
            let span = end - start;
            // Cut within one VAD frame of the 6s target (never far above it).
            assert!(
                span >= max_samples && span <= max_samples + CHUNK_SIZE_16KHZ,
                "force-cut span {span} not at the 6s target ({max_samples})"
            );
        }
        // Chunks are back-to-back (no dropped audio between them).
        assert_eq!(
            spans[1].0, spans[0].1,
            "second chunk must resume at the cut"
        );
    }

    /// Silence arriving right after a force-cut: the resumed (confirmed)
    /// chunk has zero speech frames yet - it must end via ordinary redemption
    /// without panicking, and the tiny trailing chunk must start exactly at
    /// the cut (no lost or duplicated samples).
    #[test]
    fn force_cut_followed_by_immediate_silence_redeems_cleanly() {
        let mut session =
            VadSession::new(VadChunkerConfig::dictation(Duration::from_millis(400))).unwrap();
        session.retained_audio = vec![1.0; SAMPLE_RATE * 30];

        // Enough continuous speech to trigger exactly one force-cut...
        let speech_frames = (7 * SAMPLE_RATE) / CHUNK_SIZE_16KHZ;
        let mut transitions = drive_synthetic(&mut session, 0.95, speech_frames);
        // ...then hard silence for several seconds.
        let silence_frames = (3 * SAMPLE_RATE) / CHUNK_SIZE_16KHZ;
        transitions.extend(drive_synthetic(&mut session, 0.01, silence_frames));

        let spans = speech_end_spans(&transitions);
        assert!(
            spans.len() >= 2,
            "expected the force-cut chunk plus a redeemed trailing chunk, got {}",
            spans.len()
        );
        // The trailing chunk resumes exactly at the cut point.
        assert_eq!(
            spans[1].0, spans[0].1,
            "trailing chunk must start at the cut"
        );
        // And it redeemed via silence (its end precedes the silence tail),
        // i.e. the session did not get stuck in Speech with zero frames.
        assert!(spans[1].1 > spans[1].0, "trailing chunk must be non-empty");
    }

    /// Meeting-path guard: the `speech` profile has no max-chunk cut, so the
    /// same continuous speech accumulates into one growing chunk and is NOT
    /// force-cut. Changing dictation must never regress this.
    #[test]
    fn speech_profile_does_not_force_cut_pauseless_speech() {
        let mut session =
            VadSession::new(VadChunkerConfig::speech(Duration::from_millis(400))).unwrap();
        session.retained_audio = vec![1.0; SAMPLE_RATE * 30];

        let frames = (13 * SAMPLE_RATE) / CHUNK_SIZE_16KHZ;
        let spans = speech_end_spans(&drive_synthetic(&mut session, 0.95, frames));

        assert!(
            spans.is_empty(),
            "meeting `speech` profile must not force-cut continuous speech, got {} cuts",
            spans.len()
        );
    }

    #[test]
    fn dictation_config_is_valid_and_bounded() {
        let config = VadChunkerConfig::dictation(Duration::from_millis(400));
        assert!(config.validate().is_ok());
        assert_eq!(config.max_chunk_duration, Some(Duration::from_secs(6)));
        // Lower ramp cap than the meeting profile so ordinary dictation pauses
        // redeem instead of being resisted toward a long target.
        assert!(config.max_negative_threshold < 0.80);
    }
}
