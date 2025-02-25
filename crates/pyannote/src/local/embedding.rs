use hypr_onnx::ndarray::Array2;
const MODEL_BYTES: &[u8] = include_bytes!("./data/embedding.onnx");

#[derive(Debug)]
pub struct EmbeddingExtractor {
    session: hypr_onnx::ort::session::Session,
}

impl Default for EmbeddingExtractor {
    fn default() -> Self {
        let session = hypr_onnx::load_model(MODEL_BYTES).unwrap();
        Self { session }
    }
}

impl EmbeddingExtractor {
    pub fn compute(&self, samples: &[f32]) -> Result<impl Iterator<Item = f32>, String> {
        let _features: Array2<f32> = knf_rs::compute_fbank(samples).unwrap();
        Ok(vec![0.0; 192].into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute() {
        let extractor = EmbeddingExtractor::default();
        let samples = vec![0.0; 16000];
        let embedding = extractor.compute(&samples).unwrap();
        assert_eq!(embedding.count(), 192);
    }
}
