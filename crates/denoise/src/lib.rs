use hypr_onnx::{load_model, ndarray, ort};

const MODEL_1_BYTES: &[u8] = include_bytes!("../data/model_1.onnx");
const MODEL_2_BYTES: &[u8] = include_bytes!("../data/model_2.onnx");

pub struct DTLN {
    model_1: ort::session::Session,
    model_2: ort::session::Session,
}

impl DTLN {
    pub fn new() -> Result<Self, ort::Error> {
        Ok(Self {
            model_1: load_model(&MODEL_1_BYTES)?,
            model_2: load_model(&MODEL_2_BYTES)?,
        })
    }

    // https://github.com/breizhn/DTLN-aec/blob/main/run_aec.py
    pub fn process(&self, _input: &[f32]) -> Result<Vec<f32>, ort::Error> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtln() {
        let _dtln = DTLN::new().unwrap();
    }
}
