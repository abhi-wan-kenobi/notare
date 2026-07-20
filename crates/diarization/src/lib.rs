use std::path::{Path, PathBuf};

use sherpa_rs::diarize::{Diarize, DiarizeConfig};
use sherpa_rs::embedding_manager::EmbeddingManager;
use sherpa_rs::speaker_id::{EmbeddingExtractor, ExtractorConfig};

const SAMPLE_RATE: u32 = 16000;

#[derive(thiserror::Error, Debug)]
pub enum DiarizeError {
    #[error("sherpa error: {0}")]
    Sherpa(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("model not found: {0}")]
    ModelNotFound(String),
}

impl From<eyre::Report> for DiarizeError {
    fn from(report: eyre::Report) -> Self {
        Self::Sherpa(report.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DiarizedSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker_index: i32,
}

pub struct Diarizer {
    inner: Diarize,
    segmentation_model: PathBuf,
    embedding_model: PathBuf,
}

impl Diarizer {
    pub fn new(
        segmentation_model: impl AsRef<Path>,
        embedding_model: impl AsRef<Path>,
    ) -> Result<Self, DiarizeError> {
        let segmentation_model = segmentation_model.as_ref().to_path_buf();
        let embedding_model = embedding_model.as_ref().to_path_buf();

        for path in [&segmentation_model, &embedding_model] {
            if !path.exists() {
                return Err(DiarizeError::ModelNotFound(path.display().to_string()));
            }
        }

        let inner = Diarize::new(
            &segmentation_model,
            &embedding_model,
            DiarizeConfig::default(),
        )?;

        Ok(Self {
            inner,
            segmentation_model,
            embedding_model,
        })
    }

    pub fn diarize(
        &mut self,
        samples_16k_mono: &[f32],
        num_speakers: Option<i32>,
    ) -> Result<Vec<DiarizedSegment>, DiarizeError> {
        let mut config = DiarizeConfig::default();
        if let Some(n) = num_speakers {
            config.num_clusters = Some(n);
        }

        let inner = Diarize::new(&self.segmentation_model, &self.embedding_model, config)?;
        self.inner = inner;

        let segments = self.inner.compute(samples_16k_mono.to_vec(), None)?;

        Ok(segments
            .into_iter()
            .map(|segment| DiarizedSegment {
                start_ms: seconds_to_ms(segment.start),
                end_ms: seconds_to_ms(segment.end),
                speaker_index: segment.speaker,
            })
            .collect())
    }
}

pub struct VoiceProfileIndex {
    extractor: EmbeddingExtractor,
    manager: EmbeddingManager,
    profiles: Vec<(String, Vec<f32>)>,
}

impl VoiceProfileIndex {
    pub fn new(embedding_model: impl AsRef<Path>) -> Result<Self, DiarizeError> {
        let path = embedding_model.as_ref();
        if !path.exists() {
            return Err(DiarizeError::ModelNotFound(path.display().to_string()));
        }

        let config = ExtractorConfig {
            model: path.display().to_string(),
            provider: None,
            num_threads: None,
            debug: false,
        };
        let extractor = EmbeddingExtractor::new(config)?;
        let manager = EmbeddingManager::new(extractor.embedding_size as i32);

        Ok(Self {
            extractor,
            manager,
            profiles: Vec::new(),
        })
    }

    pub fn enroll(
        &mut self,
        human_id: String,
        samples_16k_mono: &[f32],
    ) -> Result<(), DiarizeError> {
        let mut embedding = self
            .extractor
            .compute_speaker_embedding(samples_16k_mono.to_vec(), SAMPLE_RATE)?;
        self.manager
            .add(human_id.clone(), embedding.as_mut_slice())?;
        self.profiles.push((human_id, embedding));
        Ok(())
    }

    pub fn identify(
        &mut self,
        samples_16k_mono: &[f32],
        threshold: f32,
    ) -> Result<Option<String>, DiarizeError> {
        let embedding = self
            .extractor
            .compute_speaker_embedding(samples_16k_mono.to_vec(), SAMPLE_RATE)?;
        Ok(self.manager.search(&embedding, threshold))
    }

    pub fn profiles(&self) -> Vec<(String, Vec<f32>)> {
        self.profiles.clone()
    }

    pub fn from_profiles(
        embedding_model: impl AsRef<Path>,
        profiles: Vec<(String, Vec<f32>)>,
    ) -> Result<Self, DiarizeError> {
        let path = embedding_model.as_ref();
        if !path.exists() {
            return Err(DiarizeError::ModelNotFound(path.display().to_string()));
        }

        let config = ExtractorConfig {
            model: path.display().to_string(),
            provider: None,
            num_threads: None,
            debug: false,
        };
        let extractor = EmbeddingExtractor::new(config)?;
        let dimension = profiles
            .first()
            .map(|(_, embedding)| embedding.len())
            .unwrap_or(extractor.embedding_size);

        let mut manager = EmbeddingManager::new(dimension as i32);
        for (human_id, mut embedding) in profiles.clone() {
            manager.add(human_id, embedding.as_mut_slice())?;
        }

        Ok(Self {
            extractor,
            manager,
            profiles,
        })
    }
}

fn seconds_to_ms(seconds: f32) -> u64 {
    (seconds * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_to_ms_rounds_to_nearest_millisecond() {
        assert_eq!(seconds_to_ms(0.0), 0);
        assert_eq!(seconds_to_ms(1.0), 1000);
        assert_eq!(seconds_to_ms(1.2345), 1235);
        assert_eq!(seconds_to_ms(0.0004), 0);
        assert_eq!(seconds_to_ms(0.0006), 1);
    }

    #[test]
    fn diarized_segment_converts_sherpa_segment() {
        let segment = sherpa_rs::diarize::Segment {
            start: 1.5,
            end: 4.123,
            speaker: 2,
        };
        let converted = DiarizedSegment {
            start_ms: seconds_to_ms(segment.start),
            end_ms: seconds_to_ms(segment.end),
            speaker_index: segment.speaker,
        };

        assert_eq!(converted.start_ms, 1500);
        assert_eq!(converted.end_ms, 4123);
        assert_eq!(converted.speaker_index, 2);
    }
}
