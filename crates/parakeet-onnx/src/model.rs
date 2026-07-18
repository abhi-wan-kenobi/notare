// Portions derived from Meetily (https://github.com/Zackriya-Solutions/meeting-minutes),
// Copyright (c) Zackriya Solutions, MIT License.
//
// Notare changes: hypr_onnx re-exported ort/ndarray, tracing instead of log,
// int8-only fixed filenames, explicit CPU sessions with capped intra-op
// threads, and the greedy-decode arithmetic factored into pure helpers so it
// can be unit tested on synthetic logits.

use std::sync::LazyLock;

use hypr_onnx::ndarray::{self, Array, Array1, Array2, Array3, ArrayD, ArrayViewD, IxDyn};
use hypr_onnx::ort::execution_providers::{CPUExecutionProvider, ExecutionProviderDispatch};
use hypr_onnx::ort::inputs;
use hypr_onnx::ort::session::Session;
use hypr_onnx::ort::session::builder::GraphOptimizationLevel;
use hypr_onnx::ort::value::TensorRef;
use regex::Regex;

use std::fs;
use std::path::Path;

pub type DecoderState = (Array3<f32>, Array3<f32>);

pub(crate) const SUBSAMPLING_FACTOR: usize = 8;
pub(crate) const WINDOW_SIZE: f32 = 0.01;
const MAX_TOKENS_PER_STEP: usize = 3;
const TDT_DURATIONS: [usize; 5] = [0, 1, 2, 3, 4];

pub const ENCODER_FILE: &str = "encoder-model.int8.onnx";
pub const DECODER_JOINT_FILE: &str = "decoder_joint-model.int8.onnx";
pub const PREPROCESSOR_FILE: &str = "nemo128.onnx";
pub const VOCAB_FILE: &str = "vocab.txt";

static DECODE_SPACE_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"\A\s|\s\B|(\s)\b"));

#[derive(Debug, Clone)]
pub struct TimestampedResult {
    pub text: String,
    /// Second offsets (chunk-relative), one per decoded token.
    pub timestamps: Vec<f32>,
    /// Decoded token strings, `\u{2581}` already replaced with a space.
    pub tokens: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ParakeetError {
    #[error("ORT error: {0}")]
    Ort(#[from] hypr_onnx::ort::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ndarray shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),
    #[error("Model input not found: {0}")]
    InputNotFound(String),
    #[error("Model output not found: {0}")]
    OutputNotFound(String),
    #[error("Failed to get tensor shape for input: {0}")]
    TensorShape(String),
    #[error("Model mutex poisoned")]
    Poisoned,
}

pub struct ParakeetModel {
    encoder: Session,
    decoder_joint: Session,
    preprocessor: Session,
    vocab: Vec<String>,
    blank_idx: i32,
    vocab_size: usize,
}

impl ParakeetModel {
    pub fn new<P: AsRef<Path>>(model_dir: P) -> Result<Self, ParakeetError> {
        let model_dir = model_dir.as_ref();
        // Deliberately not using `hypr_onnx::load_model_from_bytes`: it pins
        // sessions to a single thread, which is far too slow for the 0.6B
        // encoder. Cap at 4 intra-op threads to stay polite on the desktop.
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(4)
            .max(1);

        // Probe GPU execution providers (if this build was compiled with
        // any) on the encoder — the biggest of the three model files, and a
        // representative proxy for whether the chosen EP works for the rest
        // of the pipeline — then reuse that decision for the other two
        // sessions instead of re-probing (GPU context init isn't free).
        let (encoder, provider) = Self::init_encoder_session(model_dir, threads)?;
        let decoder_joint = Self::init_session_with_provider(
            model_dir,
            DECODER_JOINT_FILE,
            threads,
            provider.as_ref(),
        )?;
        let preprocessor = Self::init_session_with_provider(
            model_dir,
            PREPROCESSOR_FILE,
            threads,
            provider.as_ref(),
        )?;

        let (vocab, blank_idx) = Self::load_vocab(&model_dir)?;
        let vocab_size = vocab.len();

        tracing::info!(vocab_size, blank_idx, "parakeet_vocabulary_loaded");

        Ok(Self {
            encoder,
            decoder_joint,
            preprocessor,
            vocab,
            blank_idx,
            vocab_size,
        })
    }

    /// GPU execution providers this build was compiled to try, most-preferred
    /// first. Both are opt-in via Cargo features (`cuda`, `directml`) and are
    /// always registered with `error_on_failure()` so [`init_encoder_session`]
    /// can catch a registration failure (missing driver, wrong GPU vendor,
    /// unsupported platform, ...) and fall back to CPU instead of the app
    /// crashing or silently running on an unexpected backend.
    ///
    /// DirectML is a Windows-only execution provider (it wraps DirectX 12).
    /// The `directml` Cargo feature can still be turned on to typecheck this
    /// crate on other platforms (e.g. `cargo check -p parakeet-onnx --features
    /// directml` on Linux CI), but the provider is only *constructed* under
    /// `target_os = "windows"`: ONNX Runtime's non-Windows binaries don't
    /// export DirectML's FFI entry point, so constructing it unconditionally
    /// would risk a link failure on a full (non-check) non-Windows build.
    fn gpu_execution_providers() -> Vec<(&'static str, ExecutionProviderDispatch)> {
        #[allow(unused_mut)]
        let mut providers: Vec<(&'static str, ExecutionProviderDispatch)> = Vec::new();

        #[cfg(all(feature = "directml", target_os = "windows"))]
        providers.push((
            "directml",
            hypr_onnx::ort::execution_providers::DirectMLExecutionProvider::default()
                .build()
                .error_on_failure(),
        ));

        #[cfg(feature = "cuda")]
        providers.push((
            "cuda",
            hypr_onnx::ort::execution_providers::CUDAExecutionProvider::default()
                .build()
                .error_on_failure(),
        ));

        providers
    }

    fn build_session(
        model_path: &Path,
        threads: usize,
        providers: Vec<ExecutionProviderDispatch>,
    ) -> hypr_onnx::ort::Result<Session> {
        Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_execution_providers(providers)?
            .with_parallel_execution(true)?
            .with_intra_threads(threads)?
            .commit_from_file(model_path)
    }

    fn log_session_inputs(session: &Session, file_name: &str) {
        for input in &session.inputs {
            tracing::debug!(
                model = file_name,
                input = %input.name,
                input_type = ?input.input_type,
                "parakeet_model_input"
            );
        }
    }

    /// Loads the encoder, trying each candidate GPU execution provider (in
    /// priority order) before falling back to CPU. Returns the session plus
    /// whichever provider ended up active (`None` means CPU), so the caller
    /// can reuse that decision for the decoder/preprocessor sessions.
    fn init_encoder_session(
        model_dir: &Path,
        threads: usize,
    ) -> Result<(Session, Option<(&'static str, ExecutionProviderDispatch)>), ParakeetError> {
        let model_path = model_dir.join(ENCODER_FILE);

        for (name, ep) in Self::gpu_execution_providers() {
            match Self::build_session(&model_path, threads, vec![ep.clone()]) {
                Ok(session) => {
                    tracing::info!(provider = name, "parakeet_execution_provider_active");
                    Self::log_session_inputs(&session, ENCODER_FILE);
                    return Ok((session, Some((name, ep))));
                }
                Err(error) => {
                    tracing::warn!(
                        provider = name,
                        %error,
                        "parakeet_execution_provider_unavailable_falling_back_to_cpu"
                    );
                }
            }
        }

        tracing::info!(provider = "cpu", "parakeet_execution_provider_active");
        let session = Self::build_session(
            &model_path,
            threads,
            vec![CPUExecutionProvider::default().build()],
        )?;
        Self::log_session_inputs(&session, ENCODER_FILE);
        Ok((session, None))
    }

    /// Loads `file_name` using the GPU provider already selected for the
    /// encoder (if any), falling back to CPU if that provider unexpectedly
    /// fails to register for this particular graph (never crashes).
    fn init_session_with_provider(
        model_dir: &Path,
        file_name: &str,
        threads: usize,
        provider: Option<&(&'static str, ExecutionProviderDispatch)>,
    ) -> Result<Session, ParakeetError> {
        let model_path = model_dir.join(file_name);

        if let Some((name, ep)) = provider {
            match Self::build_session(&model_path, threads, vec![ep.clone()]) {
                Ok(session) => {
                    Self::log_session_inputs(&session, file_name);
                    return Ok(session);
                }
                Err(error) => {
                    tracing::warn!(
                        model = file_name,
                        provider = %name,
                        %error,
                        "parakeet_execution_provider_unavailable_falling_back_to_cpu"
                    );
                }
            }
        }

        let session = Self::build_session(
            &model_path,
            threads,
            vec![CPUExecutionProvider::default().build()],
        )?;
        Self::log_session_inputs(&session, file_name);
        Ok(session)
    }

    fn load_vocab<P: AsRef<Path>>(model_dir: P) -> Result<(Vec<String>, i32), ParakeetError> {
        let vocab_path = model_dir.as_ref().join(VOCAB_FILE);
        let content = fs::read_to_string(vocab_path)?;
        parse_vocab(&content)
    }

    pub fn preprocess(
        &mut self,
        waveforms: &ArrayViewD<f32>,
        waveforms_lens: &ArrayViewD<i64>,
    ) -> Result<(ArrayD<f32>, ArrayD<i64>), ParakeetError> {
        tracing::trace!("parakeet_preprocessor_inference");
        let inputs = inputs![
            "waveforms" => TensorRef::from_array_view(waveforms.view())?,
            "waveforms_lens" => TensorRef::from_array_view(waveforms_lens.view())?,
        ];
        let outputs = self.preprocessor.run(inputs)?;

        let features = outputs
            .get("features")
            .ok_or_else(|| ParakeetError::OutputNotFound("features".to_string()))?
            .try_extract_array()?;
        let features_lens = outputs
            .get("features_lens")
            .ok_or_else(|| ParakeetError::OutputNotFound("features_lens".to_string()))?
            .try_extract_array()?;

        Ok((features.to_owned(), features_lens.to_owned()))
    }

    pub fn encode(
        &mut self,
        audio_signal: &ArrayViewD<f32>,
        length: &ArrayViewD<i64>,
    ) -> Result<(ArrayD<f32>, ArrayD<i64>), ParakeetError> {
        tracing::trace!("parakeet_encoder_inference");
        let inputs = inputs![
            "audio_signal" => TensorRef::from_array_view(audio_signal.view())?,
            "length" => TensorRef::from_array_view(length.view())?,
        ];
        let outputs = self.encoder.run(inputs)?;

        let encoder_output = outputs
            .get("outputs")
            .ok_or_else(|| ParakeetError::OutputNotFound("outputs".to_string()))?
            .try_extract_array()?;
        let encoded_lengths = outputs
            .get("encoded_lengths")
            .ok_or_else(|| ParakeetError::OutputNotFound("encoded_lengths".to_string()))?
            .try_extract_array()?;

        let encoder_output = encoder_output.permuted_axes(IxDyn(&[0, 2, 1]));

        Ok((encoder_output.to_owned(), encoded_lengths.to_owned()))
    }

    pub fn create_decoder_state(&self) -> Result<DecoderState, ParakeetError> {
        // Get input shapes from decoder model
        let inputs = &self.decoder_joint.inputs;

        let state1_shape = inputs
            .iter()
            .find(|input| input.name == "input_states_1")
            .ok_or_else(|| ParakeetError::InputNotFound("input_states_1".to_string()))?
            .input_type
            .tensor_shape()
            .ok_or_else(|| ParakeetError::TensorShape("input_states_1".to_string()))?;

        let state2_shape = inputs
            .iter()
            .find(|input| input.name == "input_states_2")
            .ok_or_else(|| ParakeetError::InputNotFound("input_states_2".to_string()))?
            .input_type
            .tensor_shape()
            .ok_or_else(|| ParakeetError::TensorShape("input_states_2".to_string()))?;

        // Create zero states with batch_size=1
        // Shape is [2, -1, 640] so we use [2, 1, 640] for batch_size=1
        let state1 = Array::zeros((
            state1_shape[0] as usize,
            1, // batch_size = 1
            state1_shape[2] as usize,
        ));

        let state2 = Array::zeros((
            state2_shape[0] as usize,
            1, // batch_size = 1
            state2_shape[2] as usize,
        ));

        Ok((state1, state2))
    }

    pub fn decode_step(
        &mut self,
        prev_tokens: &[i32],
        prev_state: &DecoderState,
        encoder_out: &ArrayViewD<f32>, // [time_steps, 1024]
    ) -> Result<(ArrayD<f32>, DecoderState), ParakeetError> {
        tracing::trace!("parakeet_decoder_inference");

        // Get last token or blank_idx if empty
        let target_token = prev_tokens.last().copied().unwrap_or(self.blank_idx);

        // Prepare inputs matching Python: encoder_out[None, :, None] -> [1, time_steps, 1]
        let encoder_outputs = encoder_out
            .to_owned()
            .insert_axis(ndarray::Axis(0))
            .insert_axis(ndarray::Axis(2));
        let targets = Array2::from_shape_vec((1, 1), vec![target_token])?;
        let target_length = Array1::from_vec(vec![1]);

        let inputs = inputs![
            "encoder_outputs" => TensorRef::from_array_view(encoder_outputs.view())?,
            "targets" => TensorRef::from_array_view(targets.view())?,
            "target_length" => TensorRef::from_array_view(target_length.view())?,
            "input_states_1" => TensorRef::from_array_view(prev_state.0.view())?,
            "input_states_2" => TensorRef::from_array_view(prev_state.1.view())?,
        ];

        let outputs = self.decoder_joint.run(inputs)?;

        let logits = outputs
            .get("outputs")
            .ok_or_else(|| ParakeetError::OutputNotFound("outputs".to_string()))?
            .try_extract_array()?;
        let state1 = outputs
            .get("output_states_1")
            .ok_or_else(|| ParakeetError::OutputNotFound("output_states_1".to_string()))?
            .try_extract_array()?;
        let state2 = outputs
            .get("output_states_2")
            .ok_or_else(|| ParakeetError::OutputNotFound("output_states_2".to_string()))?
            .try_extract_array()?;

        // Squeeze outputs like Python (remove batch dimension)
        let logits = logits.remove_axis(ndarray::Axis(0));

        // Convert ArrayD back to Array3 to match expected return type
        let state1_3d = state1.to_owned().into_dimensionality::<ndarray::Ix3>()?;
        let state2_3d = state2.to_owned().into_dimensionality::<ndarray::Ix3>()?;

        Ok((logits.to_owned(), (state1_3d, state2_3d)))
    }

    pub fn recognize_batch(
        &mut self,
        waveforms: &ArrayViewD<f32>,
        waveforms_len: &ArrayViewD<i64>,
    ) -> Result<Vec<TimestampedResult>, ParakeetError> {
        // Preprocess and encode
        let (features, features_lens) = self.preprocess(waveforms, waveforms_len)?;
        let (encoder_out, encoder_out_lens) =
            self.encode(&features.view(), &features_lens.view())?;

        // Decode for each batch item
        let mut results = Vec::new();
        for (encodings, &encodings_len) in encoder_out.outer_iter().zip(encoder_out_lens.iter()) {
            let (tokens, timestamps) =
                self.decode_sequence(&encodings.view(), encodings_len as usize)?;
            let result = self.decode_tokens(tokens, timestamps);
            results.push(result);
        }

        Ok(results)
    }

    fn decode_sequence(
        &mut self,
        encodings: &ArrayViewD<f32>, // [time_steps, 1024]
        encodings_len: usize,
    ) -> Result<(Vec<i32>, Vec<usize>), ParakeetError> {
        let mut prev_state = self.create_decoder_state()?;
        let mut tokens = Vec::new();
        let mut timestamps = Vec::new();

        let mut t = 0;
        let mut emitted_tokens = 0;

        while t < encodings_len {
            let encoder_step = encodings.slice(ndarray::s![t, ..]);
            // Convert to dynamic dimension to match decode_step parameter type
            let encoder_step_dyn = encoder_step.to_owned().into_dyn();
            let (probs, new_state) =
                self.decode_step(&tokens, &prev_state, &encoder_step_dyn.view())?;

            // For TDT models, split output into vocab logits and duration logits
            // output[:vocab_size] = vocabulary logits
            // output[vocab_size:] = duration logits
            let vocab_logits_slice = probs.as_slice().ok_or_else(|| {
                ParakeetError::Shape(ndarray::ShapeError::from_kind(
                    ndarray::ErrorKind::IncompatibleShape,
                ))
            })?;

            let is_tdt = probs.len() > self.vocab_size;
            let (vocab_logits, duration_logits) = if is_tdt {
                let (v, d) = vocab_logits_slice.split_at(self.vocab_size);
                (v, Some(d))
            } else {
                (vocab_logits_slice, None)
            };

            // Get argmax token from vocabulary logits only
            let token = argmax(vocab_logits)
                .map(|idx| idx as i32)
                .unwrap_or(self.blank_idx);

            let is_blank = token == self.blank_idx;
            if !is_blank {
                prev_state = new_state;
                tokens.push(token);
                timestamps.push(t);
                emitted_tokens += 1;
            }

            let skip = greedy_advance(is_blank, emitted_tokens, duration_logits);
            if skip > 0 {
                t += skip;
                emitted_tokens = 0;
            }
        }

        if tokens.is_empty() {
            tracing::debug!(
                encodings_len,
                "parakeet_decoded_zero_tokens (all blank) - audio may be too short or low energy"
            );
        }

        Ok((tokens, timestamps))
    }

    fn decode_tokens(&self, ids: Vec<i32>, timestamps: Vec<usize>) -> TimestampedResult {
        let tokens: Vec<String> = ids
            .iter()
            .filter_map(|&id| {
                let idx = id as usize;
                if idx < self.vocab.len() {
                    Some(self.vocab[idx].clone())
                } else {
                    None
                }
            })
            .collect();

        let text = match &*DECODE_SPACE_RE {
            Ok(regex) => regex
                .replace_all(&tokens.join(""), |caps: &regex::Captures| {
                    if caps.get(1).is_some() { " " } else { "" }
                })
                .to_string(),
            Err(_) => tokens.join(""), // Fallback if regex failed to compile
        };

        let float_timestamps: Vec<f32> = timestamps
            .iter()
            .map(|&t| WINDOW_SIZE * SUBSAMPLING_FACTOR as f32 * t as f32)
            .collect();

        TimestampedResult {
            text,
            timestamps: float_timestamps,
            tokens,
        }
    }

    pub fn transcribe_samples(
        &mut self,
        samples: Vec<f32>,
    ) -> Result<TimestampedResult, ParakeetError> {
        let batch_size = 1;
        let samples_len = samples.len();

        // Create waveforms array [batch_size, samples_len]
        let waveforms = Array2::from_shape_vec((batch_size, samples_len), samples)?.into_dyn();

        // Create waveforms_lens array [batch_size] with the actual length
        let waveforms_lens = Array1::from_vec(vec![samples_len as i64]).into_dyn();

        // Run recognition to get detailed results
        let results = self.recognize_batch(&waveforms.view(), &waveforms_lens.view())?;

        // Extract the first (and only) result
        let timestamped_result = results.into_iter().next().ok_or_else(|| {
            ParakeetError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No transcription result returned",
            ))
        })?;

        Ok(timestamped_result)
    }
}

/// Parse a NeMo-style `vocab.txt` (`<token> <id>` per line) into an
/// id-indexed token table (`\u{2581}` mapped to a space) plus the blank id.
pub(crate) fn parse_vocab(content: &str) -> Result<(Vec<String>, i32), ParakeetError> {
    let mut max_id = 0;
    let mut tokens_with_ids: Vec<(String, usize)> = Vec::new();
    let mut blank_idx: Option<usize> = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.trim_end().split(' ').collect();
        if parts.len() >= 2 {
            let token = parts[0].to_string();
            if let Ok(id) = parts[1].parse::<usize>() {
                if token == "<blk>" {
                    blank_idx = Some(id);
                }
                tokens_with_ids.push((token, id));
                max_id = max_id.max(id);
            }
        }
    }

    // Create vocab vector with \u{2581} replaced with space
    let mut vocab = vec![String::new(); max_id + 1];
    for (token, id) in tokens_with_ids {
        vocab[id] = token.replace('\u{2581}', " ");
    }

    let blank_idx = blank_idx.ok_or_else(|| {
        ParakeetError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Missing <blk> token in vocabulary",
        ))
    })? as i32;

    Ok((vocab, blank_idx))
}

pub(crate) fn argmax(values: &[f32]) -> Option<usize> {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
}

/// How many encoder frames the greedy decode should advance after one step.
///
/// TDT: advance by the model's predicted duration, but force forward progress
/// on blank-with-zero-duration and cap same-frame emissions to avoid runaway
/// repetition. RNN-T: advance one frame on blank or after the emission cap.
/// A returned `0` means "stay on this frame" (another token may be emitted).
pub(crate) fn greedy_advance(
    is_blank: bool,
    emitted_tokens: usize,
    duration_logits: Option<&[f32]>,
) -> usize {
    match duration_logits {
        Some(duration_logits) => {
            let dur_idx = argmax(duration_logits).unwrap_or(0);
            let mut skip = TDT_DURATIONS.get(dur_idx).copied().unwrap_or(1);

            if skip == 0 && (is_blank || emitted_tokens >= MAX_TOKENS_PER_STEP) {
                skip = 1;
            }
            skip
        }
        None => {
            if is_blank || emitted_tokens >= MAX_TOKENS_PER_STEP {
                1
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vocab_maps_ids_and_finds_blank() {
        let content = "\u{2581}hello 0\nworld 1\n<blk> 2\n\u{2581}a 3\n";
        let (vocab, blank_idx) = parse_vocab(content).unwrap();

        assert_eq!(blank_idx, 2);
        assert_eq!(vocab.len(), 4);
        assert_eq!(vocab[0], " hello");
        assert_eq!(vocab[1], "world");
        assert_eq!(vocab[2], "<blk>");
        assert_eq!(vocab[3], " a");
    }

    #[test]
    fn parse_vocab_without_blank_is_an_error() {
        let content = "\u{2581}hello 0\nworld 1\n";
        assert!(parse_vocab(content).is_err());
    }

    #[test]
    fn parse_vocab_ignores_malformed_lines() {
        let content = "justonetoken\n<blk> 0\nbad id\nok 1\n";
        let (vocab, blank_idx) = parse_vocab(content).unwrap();
        assert_eq!(blank_idx, 0);
        assert_eq!(vocab[1], "ok");
    }

    #[test]
    fn argmax_picks_largest_and_handles_empty() {
        assert_eq!(argmax(&[0.1, 3.0, 2.0]), Some(1));
        assert_eq!(argmax(&[]), None);
    }

    #[test]
    fn tdt_advance_follows_predicted_duration() {
        // duration argmax = index 3 -> skip 3 frames
        let durations = [0.0, 0.1, 0.2, 5.0, 0.3];
        assert_eq!(greedy_advance(false, 1, Some(&durations)), 3);
        assert_eq!(greedy_advance(true, 0, Some(&durations)), 3);
    }

    #[test]
    fn tdt_advance_forces_forward_progress_on_blank_zero_duration() {
        // duration argmax = index 0 -> predicted skip 0
        let durations = [5.0, 0.1, 0.2, 0.3, 0.4];
        // blank + zero duration must still advance
        assert_eq!(greedy_advance(true, 0, Some(&durations)), 1);
        // non-blank below the cap may stay on the frame
        assert_eq!(greedy_advance(false, 1, Some(&durations)), 0);
    }

    #[test]
    fn tdt_advance_caps_same_frame_emissions() {
        let durations = [5.0, 0.1, 0.2, 0.3, 0.4];
        // at the emission cap, zero-duration must advance
        assert_eq!(
            greedy_advance(false, MAX_TOKENS_PER_STEP, Some(&durations)),
            1
        );
    }

    #[test]
    fn rnnt_advance_moves_on_blank_or_cap_only() {
        assert_eq!(greedy_advance(true, 0, None), 1);
        assert_eq!(greedy_advance(false, 1, None), 0);
        assert_eq!(greedy_advance(false, MAX_TOKENS_PER_STEP, None), 1);
    }

    #[test]
    fn tdt_decode_makes_forward_progress_on_synthetic_logits() {
        // Simulate the decode_sequence loop arithmetic over synthetic frames
        // where the model always emits a non-blank token with duration 0:
        // the MAX_TOKENS_PER_STEP cap must still guarantee termination.
        let durations = [5.0_f32, 0.0, 0.0, 0.0, 0.0]; // predicted skip = 0
        let encodings_len = 10usize;
        let mut t = 0usize;
        let mut emitted_tokens = 0usize;
        let mut iterations = 0usize;

        while t < encodings_len {
            iterations += 1;
            assert!(
                iterations <= encodings_len * (MAX_TOKENS_PER_STEP + 1),
                "decode loop failed to terminate"
            );

            // non-blank emission every step
            emitted_tokens += 1;
            let skip = greedy_advance(false, emitted_tokens, Some(&durations));
            if skip > 0 {
                t += skip;
                emitted_tokens = 0;
            }
        }

        assert_eq!(t, encodings_len);
        // exactly MAX_TOKENS_PER_STEP emissions happen per frame + 1 forced advance
        assert_eq!(iterations, encodings_len * MAX_TOKENS_PER_STEP);
    }

    #[test]
    fn gpu_execution_providers_only_include_compiled_in_backends() {
        let providers = ParakeetModel::gpu_execution_providers();
        let names: Vec<&str> = providers.iter().map(|(name, _)| *name).collect();

        #[cfg(not(feature = "cuda"))]
        assert!(
            !names.contains(&"cuda"),
            "the cuda EP must not be attempted when the `cuda` feature is off"
        );
        #[cfg(feature = "cuda")]
        assert!(
            names.contains(&"cuda"),
            "the cuda EP must be attempted when the `cuda` feature is on"
        );

        #[cfg(not(all(feature = "directml", target_os = "windows")))]
        assert!(
            !names.contains(&"directml"),
            "the directml EP must only be attempted with the `directml` feature on AND target_os = windows"
        );
        #[cfg(all(feature = "directml", target_os = "windows"))]
        assert!(
            names.contains(&"directml"),
            "the directml EP must be attempted on Windows when the `directml` feature is on"
        );
    }

    #[test]
    fn directml_is_never_constructed_off_windows_even_with_the_feature_on() {
        // Regression guard for the link-safety property `gpu_execution_providers`
        // documents: constructing `DirectMLExecutionProvider` off Windows risks
        // a link failure on a full (non-`cargo check`) build, because ONNX
        // Runtime's non-Windows binaries don't export DirectML's FFI entry
        // point. Even with the `directml` Cargo feature on (e.g. to typecheck
        // this crate on Linux CI), the provider must never actually be built
        // outside `target_os = "windows"`.
        #[cfg(not(target_os = "windows"))]
        {
            let providers = ParakeetModel::gpu_execution_providers();
            assert!(
                providers.iter().all(|(name, _)| *name != "directml"),
                "directml must never be constructed off Windows"
            );
        }
    }

    #[test]
    fn default_build_has_no_gpu_candidates() {
        // With neither the `cuda` nor `directml` feature on (the plain `cargo
        // test -p parakeet-onnx` gate), the model must fall straight to CPU —
        // byte-for-byte the pre-GPU-feature behavior.
        #[cfg(not(any(feature = "cuda", feature = "directml")))]
        assert!(ParakeetModel::gpu_execution_providers().is_empty());
    }
}
