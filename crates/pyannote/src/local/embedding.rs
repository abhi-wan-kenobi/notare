use hypr_onnx::{
    ndarray::{self, Array2},
    ort::{self, session::Session},
};

const EMBEDDING_ONNX: &[u8] = include_bytes!("./data/embedding.onnx");

pub struct EmbeddingExtractor {
    session: Session,
}

impl EmbeddingExtractor {
    pub fn new() -> Self {
        let session = hypr_onnx::load_model(EMBEDDING_ONNX).unwrap();
        Self { session }
    }

    pub fn compute(&mut self, samples: &[i16]) -> Result<Vec<f32>, anyhow::Error> {
        let mut samples_f32 = vec![0.0; samples.len()];
        knf_rs::convert_integer_to_float_audio(samples, &mut samples_f32);
        let samples = &samples_f32;

        let features: Array2<f32> = knf_rs::compute_fbank(samples).unwrap();
        let features = features.insert_axis(ndarray::Axis(0));
        let inputs = ort::inputs! ["feats" => features.view()]?;

        let ort_outs = self.session.run(inputs)?;
        let ort_out = ort_outs
            .get("embs")
            .unwrap()
            .try_extract_tensor::<f32>()
            .unwrap();

        let embeddings = ort_out.iter().copied().collect::<Vec<_>>();
        Ok(embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_audio(path: &str) -> Vec<i16> {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let p = base.join("src/local/data").join(path);

        let audio = rodio::Decoder::new(std::io::BufReader::new(std::fs::File::open(p).unwrap()))
            .unwrap()
            .collect::<Vec<_>>();

        audio
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        dot_product / (norm_a * norm_b)
    }

    #[test]
    fn test_embedding_extractor() {
        use simsimd::SpatialSimilarity;

        let mut extractor = EmbeddingExtractor::new();

        let female_1 = extractor
            .compute(&get_audio("female_welcome_1.mp3"))
            .unwrap();
        let male_1 = extractor.compute(&get_audio("male_welcome_1.mp3")).unwrap();
        let male_2 = extractor.compute(&get_audio("male_welcome_2.mp3")).unwrap();

        assert_eq!(female_1.len(), male_1.len());
        assert_eq!(female_1.len(), male_2.len());

        assert!(
            (1.0 - f32::cosine(&female_1, &male_1).unwrap())
                < (1.0 - f32::cosine(&male_1, &male_2).unwrap())
        );
        assert!(
            (1.0 - f32::cosine(&female_1, &male_2).unwrap())
                < (1.0 - f32::cosine(&male_2, &male_1).unwrap())
        );
    }
}
