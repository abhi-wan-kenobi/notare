use hypr_onnx::ndarray::{self, Array, Array1, Array2, ArrayD, IxDyn};
use hypr_onnx::ort::{
    session::{Session, SessionInputValue, SessionInputs},
    value::Value,
};
use std::collections::HashMap;

use crate::Error;

pub struct MoonshineOnnxModel {
    encoder: Session,
    decoder: Session,

    // model hyperparams derived from model name
    num_layers: usize,
    num_key_value_heads: usize,
    head_dim: usize,

    decoder_start_token_id: i64,
    eos_token_id: i64,
}

impl MoonshineOnnxModel {
    pub fn from_paths(
        encoder_path: impl AsRef<std::path::Path>,
        decoder_path: impl AsRef<std::path::Path>,
        model_name: &str,
    ) -> Result<Self, Error> {
        let encoder_bytes = std::fs::read(encoder_path).unwrap();
        let decoder_bytes = std::fs::read(decoder_path).unwrap();
        let encoder = hypr_onnx::load_model_from_bytes(encoder_bytes.as_ref()).unwrap();
        let decoder = hypr_onnx::load_model_from_bytes(decoder_bytes.as_ref()).unwrap();

        let (num_layers, num_key_value_heads, head_dim) = match model_name {
            name if name.contains("tiny") => (6, 8, 36),
            name if name.contains("base") => (8, 8, 52),
            other => return Err(Error::InvalidModelName(other.to_string())),
        };

        Ok(Self {
            encoder,
            decoder,
            num_layers,
            num_key_value_heads,
            head_dim,
            decoder_start_token_id: 1,
            eos_token_id: 2,
        })
    }

    // audio shape: [1, num_audio_samples], 16kHz
    pub fn generate(
        &mut self,
        audio: Array2<f32>,
        max_len: Option<usize>,
    ) -> Result<Vec<i64>, Error> {
        let nsamp = audio.shape().get(1).copied().unwrap_or(0);
        let mut max_len = max_len.unwrap_or_else(|| ((nsamp as f32 / 16_000.0) * 6.0) as usize);
        if max_len == 0 {
            max_len = 1;
        }

        // Run encoder: input name "input_values" - returns last_hidden_state
        let audio_value = Value::from_array(audio)?;
        let enc_inputs =
            SessionInputs::from(vec![("input_values", SessionInputValue::from(audio_value))]);

        let mut enc_outputs = self.encoder.run(enc_inputs)?;

        // Extract outputs by name - we expect one output from encoder
        let output_names: Vec<String> = enc_outputs.keys().map(|k| k.to_string()).collect();
        if output_names.is_empty() {
            return Err(Error::Shape("encoder produced no outputs".into()));
        }

        let last_hidden_state_value = enc_outputs
            .remove(&output_names[0])
            .ok_or_else(|| Error::Shape("failed to get encoder output".into()))?;

        // Extract tensor data and shape
        let (shape, data) = last_hidden_state_value
            .try_extract_tensor::<f32>()
            .map_err(|_| Error::Shape("failed to extract encoder output as f32 tensor".into()))?;

        // Convert to ndarray
        let shape_vec: Vec<usize> = shape.iter().map(|&x| x as usize).collect();
        if shape_vec.len() != 3 {
            return Err(Error::Shape(format!(
                "expected 3D tensor, got {:?}",
                shape_vec
            )));
        }
        let last_hidden_state = ndarray::Array3::from_shape_vec(
            (shape_vec[0], shape_vec[1], shape_vec[2]),
            data.to_vec(),
        )
        .map_err(|e| Error::Shape(format!("failed to create array3: {e}")))?;

        // Build initial past_key_values with zeros: shape (0, num_heads, 1, head_dim)
        let mut past: HashMap<String, Value> = HashMap::new();
        for i in 0..self.num_layers {
            for a in ["decoder", "encoder"] {
                for b in ["key", "value"] {
                    let key = format!("past_key_values.{i}.{a}.{b}");
                    // zeros with shape (0, num_heads, 1, head_dim)
                    let zeros: ArrayD<f32> =
                        ArrayD::zeros(IxDyn(&[0, self.num_key_value_heads, 1, self.head_dim]));
                    // Create a dynamic value that can be stored
                    let value = Value::from_array(zeros)?;
                    past.insert(key, value.into_dyn());
                }
            }
        }

        let mut tokens: Vec<i64> = vec![self.decoder_start_token_id];
        // ONNX expects rank-2 input_ids [batch, seq]
        let mut input_ids =
            ndarray::Array2::from_shape_vec((1, 1), vec![self.decoder_start_token_id])
                .map_err(|e| Error::Shape(format!("input_ids init: {e}")))?;

        for i in 0..max_len {
            let use_cache_branch = i > 0;

            // Build decoder inputs
            let mut dec_input_vec: Vec<(&str, SessionInputValue)> = vec![];

            // Add main inputs
            dec_input_vec.push((
                "input_ids",
                SessionInputValue::from(Value::from_array(input_ids.clone())?),
            ));
            dec_input_vec.push((
                "encoder_hidden_states",
                SessionInputValue::from(Value::from_array(last_hidden_state.clone())?),
            ));

            // use_cache_branch as boolean array
            let ucb = Array1::from_vec(vec![if use_cache_branch { 1_i64 } else { 0_i64 }]);
            dec_input_vec.push((
                "use_cache_branch",
                SessionInputValue::from(Value::from_array(ucb)?),
            ));

            // Add past key values - need to re-create values since they can't be cloned
            let past_keys: Vec<String> = past.keys().cloned().collect();
            for k in past_keys {
                if let Some(v) = past.get(&k) {
                    // Extract tensor data and shape, then re-wrap
                    let (shape, data) = v
                        .try_extract_tensor::<f32>()
                        .map_err(|_| Error::Shape("failed to extract past kv tensor".into()))?;
                    let shape_vec: Vec<usize> = shape.iter().map(|&x| x as usize).collect();
                    let tensor =
                        ArrayD::from_shape_vec(IxDyn(&shape_vec), data.to_vec()).map_err(|e| {
                            Error::Shape(format!("failed to recreate past tensor: {e}"))
                        })?;
                    let new_value = Value::from_array(tensor)?.into_dyn();
                    let leaked_key = Box::leak(k.clone().into_boxed_str());
                    dec_input_vec.push((leaked_key, SessionInputValue::from(new_value)));
                }
            }

            let dec_inputs = SessionInputs::from(dec_input_vec);
            let outputs = self.decoder.run(dec_inputs)?;

            // Get decoder outputs
            let mut dec_outputs = outputs;
            let output_names: Vec<String> = dec_outputs.keys().map(|k| k.to_string()).collect();
            if output_names.is_empty() {
                return Err(Error::Shape("decoder produced no outputs".into()));
            }

            // First output should be logits
            let logits_value = dec_outputs
                .remove(&output_names[0])
                .ok_or_else(|| Error::Shape("failed to get logits output".into()))?;

            let (shape, data) = logits_value
                .try_extract_tensor::<f32>()
                .map_err(|_| Error::Shape("failed to extract logits as f32 tensor".into()))?;

            let shape_vec: Vec<usize> = shape.iter().map(|&x| x as usize).collect();
            if shape_vec.len() != 3 {
                return Err(Error::Shape(format!(
                    "expected 3D logits, got {:?}",
                    shape_vec
                )));
            }
            let logits = ndarray::Array3::from_shape_vec(
                (shape_vec[0], shape_vec[1], shape_vec[2]),
                data.to_vec(),
            )
            .map_err(|e| Error::Shape(format!("failed to create logits array3: {e}")))?;

            // argmax over vocab at last position
            let last_pos = logits.shape()[1] - 1;
            let next = argmax_1d(logits.slice(ndarray::s![0, last_pos, ..]).to_owned());
            tokens.push(next);
            if next == self.eos_token_id {
                break;
            }

            // prepare next iteration
            input_ids = ndarray::Array2::from_shape_vec((1, 1), vec![next])
                .map_err(|e| Error::Shape(format!("input_ids next: {e}")))?;

            // Update past key values from outputs
            // The remaining outputs are the present_key_values
            let past_keys: Vec<String> = past.keys().cloned().collect();
            for (idx, k) in past_keys.iter().enumerate() {
                if idx + 1 < output_names.len() {
                    let present_key = &output_names[idx + 1]; // +1 because first output is logits
                    if let Some(present) = dec_outputs.remove(present_key) {
                        let should_update = !use_cache_branch || k.contains(".decoder.");
                        if should_update {
                            past.insert(k.clone(), present.into_dyn());
                        }
                    }
                }
            }
        }

        Ok(tokens)
    }
}

fn argmax_1d(v: Array1<f32>) -> i64 {
    let mut max_idx = 0usize;
    let mut max_val = f32::NEG_INFINITY;
    for (i, val) in v.iter().enumerate() {
        if *val > max_val {
            max_val = *val;
            max_idx = i;
        }
    }
    max_idx as i64
}
