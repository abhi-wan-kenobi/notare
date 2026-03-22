use std::time::Duration;

use hypr_onnx::ndarray::ArrayView1;
use hypr_vad::silero_onnx::{CHUNK_SIZE_16KHZ, SileroVad};

const SAMPLE_RATE: usize = 16000;

pub struct AdaptiveVadConfig {
    pub positive_speech_threshold: f32,
    pub negative_speech_threshold: f32,
    pub redemption_time: Duration,
    pub pre_speech_pad: Duration,
    pub min_speech_time: Duration,
    pub min_chunk_duration: Duration,
    pub target_chunk_duration: Duration,
    pub max_negative_threshold: f32,
}

impl Default for AdaptiveVadConfig {
    fn default() -> Self {
        Self {
            positive_speech_threshold: 0.5,
            negative_speech_threshold: 0.35,
            redemption_time: Duration::from_millis(600),
            pre_speech_pad: Duration::from_millis(600),
            min_speech_time: Duration::from_millis(90),
            min_chunk_duration: Duration::from_secs(3),
            target_chunk_duration: Duration::from_secs(15),
            max_negative_threshold: 0.70,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum VadTransition {
    SpeechStart {
        timestamp_ms: usize,
    },
    SpeechEnd {
        start_timestamp_ms: usize,
        end_timestamp_ms: usize,
        samples: Vec<f32>,
    },
}

#[derive(Clone, Copy)]
enum VadState {
    Silence,
    Speech {
        start_ms: usize,
        confirmed: bool,
        speech_samples: usize,
    },
}

pub struct AdaptiveVadSession {
    silero: SileroVad,
    config: AdaptiveVadConfig,
    state: VadState,
    session_audio: Vec<f32>,
    cursor: usize,
    silent_samples: usize,
    last_prob: f32,
}

impl AdaptiveVadSession {
    pub fn new(config: AdaptiveVadConfig) -> Result<Self, crate::Error> {
        let silero = SileroVad::default();
        Ok(Self {
            silero,
            config,
            state: VadState::Silence,
            session_audio: Vec::new(),
            cursor: 0,
            silent_samples: 0,
            last_prob: 0.0,
        })
    }

    pub fn last_probability(&self) -> f32 {
        self.last_prob
    }

    pub fn is_speaking(&self) -> bool {
        matches!(
            self.state,
            VadState::Speech {
                confirmed: true,
                ..
            }
        )
    }

    pub fn speech_duration(&self) -> Duration {
        match &self.state {
            VadState::Speech { speech_samples, .. } => {
                Duration::from_millis((*speech_samples * 1000 / SAMPLE_RATE) as u64)
            }
            VadState::Silence => Duration::ZERO,
        }
    }

    fn cursor_ms(&self) -> usize {
        self.cursor * 1000 / SAMPLE_RATE
    }

    fn silence_ms(&self) -> usize {
        self.silent_samples * 1000 / SAMPLE_RATE
    }

    pub(crate) fn process(
        &mut self,
        audio_frame: &[f32],
    ) -> Result<Vec<VadTransition>, crate::Error> {
        self.session_audio.extend_from_slice(audio_frame);

        let mut transitions = Vec::new();

        while self.session_audio.len() - self.cursor >= CHUNK_SIZE_16KHZ {
            let chunk =
                ArrayView1::from(&self.session_audio[self.cursor..self.cursor + CHUNK_SIZE_16KHZ]);

            let prob = self
                .silero
                .process_chunk(&chunk, 16000)
                .map_err(|e| crate::Error::VadProcessingFailed(e.to_string()))?;
            self.last_prob = prob;
            self.cursor += CHUNK_SIZE_16KHZ;

            if let Some(t) = self.advance(prob) {
                transitions.push(t);
            }
        }

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
                    let pad_ms = self.config.pre_speech_pad.as_millis() as usize;
                    let start_ms = self.cursor_ms().saturating_sub(pad_ms);
                    self.state = VadState::Speech {
                        start_ms,
                        confirmed: false,
                        speech_samples: CHUNK_SIZE_16KHZ,
                    };
                    self.silent_samples = 0;
                }
                None
            }
            VadState::Speech {
                start_ms,
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

                let min_speech_ms = self.config.min_speech_time.as_millis() as usize;
                let speech_ms = speech_samples * 1000 / SAMPLE_RATE;
                let silence_ms = self.silence_ms();
                let redemption_ms = self.config.redemption_time.as_millis() as usize;

                if !confirmed && speech_ms >= min_speech_ms {
                    self.state = VadState::Speech {
                        start_ms,
                        confirmed: true,
                        speech_samples,
                    };
                    return Some(VadTransition::SpeechStart {
                        timestamp_ms: start_ms,
                    });
                }

                if confirmed && silence_ms >= redemption_ms {
                    let speech_end_ms = (self.cursor - self.silent_samples) * 1000 / SAMPLE_RATE;

                    let start_idx = start_ms * SAMPLE_RATE / 1000;
                    let end_idx =
                        (speech_end_ms * SAMPLE_RATE / 1000).min(self.session_audio.len());
                    let samples = self.session_audio[start_idx..end_idx].to_vec();

                    self.state = VadState::Silence;
                    self.silent_samples = 0;

                    return Some(VadTransition::SpeechEnd {
                        start_timestamp_ms: start_ms,
                        end_timestamp_ms: speech_end_ms,
                        samples,
                    });
                }

                if !confirmed && silence_ms >= redemption_ms {
                    self.state = VadState::Silence;
                    self.silent_samples = 0;
                } else {
                    self.state = VadState::Speech {
                        start_ms,
                        confirmed,
                        speech_samples,
                    };
                }

                None
            }
        }
    }
}
