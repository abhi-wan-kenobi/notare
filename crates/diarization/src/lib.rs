use std::path::{Path, PathBuf};

use sherpa_rs::diarize::{Diarize, DiarizeConfig};
use sherpa_rs::embedding_manager::EmbeddingManager;
use sherpa_rs::speaker_id::{EmbeddingExtractor, ExtractorConfig};

const SAMPLE_RATE: u32 = 16000;

/// Sentinel passed to sherpa's `num_clusters` to select threshold-based auto
/// clustering (`cutree_cdist`) instead of a fixed-K cut. Any value `<= 0` works;
/// -1 is the sherpa-onnx convention for "decide the count from the data".
const AUTO_CLUSTER_NUM_CLUSTERS: i32 = -1;

/// Cosine-distance cutoff for auto speaker clustering (sherpa `FastClustering`).
/// Only used when no explicit speaker count is supplied. Lower ⇒ splits into more
/// speakers; higher ⇒ merges. Calibrated against the labelled `hypr-data` fixtures
/// (see `examples/threshold_sweep.rs`): sherpa's struct default of 0.5 badly
/// over-splits (a clean 2-speaker clip → 4-9 speakers); 0.99 recovers the right
/// count on the 3-speaker clip and lands within +1 on the 2-speaker clips. Auto
/// is best-effort — the P2.6 "# of speakers" control provides an exact override.
const AUTO_CLUSTER_THRESHOLD: f32 = 0.99;

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

fn cluster_config(num_speakers: Option<i32>) -> (Option<i32>, Option<f32>) {
    match num_speakers {
        // An explicit, positive speaker count → a hard cluster count
        // (sherpa's fixed-K cut, `cutree_k`): exactly `n` speakers.
        Some(n) if n > 0 => (Some(n), None),
        // No usable hint → auto-detect the number of speakers from the audio.
        // sherpa-rs's `DiarizeConfig::default()` hard-codes
        // `num_clusters = Some(4)`, and `Diarize::new` does
        // `num_clusters.unwrap_or(4)`, so leaving it as-is (or `None`) forces
        // a fixed cut of 4 and silently ignores `threshold` — the bug that
        // capped every recording at ≤4 speakers. Passing `-1` selects
        // sherpa's threshold-based clustering (`cutree_cdist`), which lets the
        // dendrogram cut where the audio actually separates speakers.
        _ => (
            Some(AUTO_CLUSTER_NUM_CLUSTERS),
            Some(AUTO_CLUSTER_THRESHOLD),
        ),
    }
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
        let (num_clusters, threshold) = cluster_config(num_speakers);
        let mut config = DiarizeConfig::default();
        config.num_clusters = num_clusters;
        config.threshold = threshold;

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

// --- Bundled models -------------------------------------------------------
//
// sherpa-onnx needs model *file paths*, so the bundled bytes are materialized
// into a cache dir once and reused. Models: pyannote-segmentation-3.0 (MIT) +
// a 3D-Speaker embedding (permissive) — the exact pair validated in the #15
// spike (11-36x realtime on CPU). ~35MB, in line with the crates/pyannote-local
// bundle already in-tree.

const SEGMENTATION_ONNX: &[u8] = include_bytes!("../data/segmentation.onnx");
const EMBEDDING_ONNX: &[u8] = include_bytes!("../data/embedding.onnx");

/// Materialize the bundled ONNX models into a cache directory and return their
/// paths. Written once; reused when the file already exists at the right size.
pub fn bundled_model_paths() -> Result<(PathBuf, PathBuf), DiarizeError> {
    let dir = std::env::temp_dir().join("notare-diarization-models");
    std::fs::create_dir_all(&dir)?;
    let segmentation = dir.join("segmentation.onnx");
    let embedding = dir.join("embedding.onnx");
    materialize(&segmentation, SEGMENTATION_ONNX)?;
    materialize(&embedding, EMBEDDING_ONNX)?;
    Ok((segmentation, embedding))
}

fn materialize(path: &Path, bytes: &[u8]) -> Result<(), DiarizeError> {
    let up_to_date = std::fs::metadata(path)
        .map(|m| m.len() == bytes.len() as u64)
        .unwrap_or(false);
    if !up_to_date {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

impl Diarizer {
    /// Construct a diarizer from the bundled segmentation + embedding models.
    pub fn from_bundled() -> Result<Self, DiarizeError> {
        let (segmentation, embedding) = bundled_model_paths()?;
        Self::new(segmentation, embedding)
    }
}

impl VoiceProfileIndex {
    /// Construct a voice-profile index from the bundled embedding model.
    pub fn from_bundled() -> Result<Self, DiarizeError> {
        let (_segmentation, embedding) = bundled_model_paths()?;
        Self::new(embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_config_uses_fixed_count_for_positive_speaker_counts() {
        assert_eq!(cluster_config(Some(5)), (Some(5), None));
        assert_eq!(cluster_config(Some(1)), (Some(1), None));
    }

    #[test]
    fn cluster_config_falls_back_to_auto_clustering() {
        let auto = (
            Some(AUTO_CLUSTER_NUM_CLUSTERS),
            Some(AUTO_CLUSTER_THRESHOLD),
        );
        assert_eq!(cluster_config(None), auto);
        assert_eq!(cluster_config(Some(0)), auto);
        assert_eq!(cluster_config(Some(-3)), auto);
    }

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

    #[test]
    fn bundled_models_materialize_and_load() {
        // Materializing writes both files to the cache dir...
        let (segmentation, embedding) = bundled_model_paths().expect("materialize");
        assert!(segmentation.exists() && embedding.exists());

        // ...and sherpa can actually parse/load them (proves the bundle is a
        // valid segmentation + embedding pair, not just bytes on disk).
        Diarizer::from_bundled().expect("diarizer loads bundled models");
        VoiceProfileIndex::from_bundled().expect("index loads bundled embedding");
    }
}
