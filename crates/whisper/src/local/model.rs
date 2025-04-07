// https://github.com/tazz4843/whisper-rs/blob/master/examples/audio_transcription.rs

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    WhisperToken,
};

pub struct Whisper {
    state: WhisperState,
    prompt: String,
    eot: WhisperToken,
}

impl Whisper {
    pub fn new(model_path: impl AsRef<str>) -> Self {
        {
            unsafe extern "C" fn noop_callback(
                _level: whisper_rs::whisper_rs_sys::ggml_log_level,
                _text: *const ::std::os::raw::c_char,
                _user_data: *mut ::std::os::raw::c_void,
            ) {
            }
            unsafe { whisper_rs::set_log_callback(Some(noop_callback), std::ptr::null_mut()) };
        }

        let context_param = {
            let mut p = WhisperContextParameters::default();
            p.dtw_parameters.mode = whisper_rs::DtwMode::None;
            p
        };

        let ctx = WhisperContext::new_with_params(model_path.as_ref(), context_param).unwrap();
        let state = ctx.create_state().unwrap();
        let eot = ctx.token_eot();

        Self {
            state,
            prompt: "".to_string(),
            eot,
        }
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> Vec<Segment> {
        let params = {
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_translate(false);
            p.set_language(Some("en"));
            p.set_initial_prompt(self.prompt.as_str());

            p.set_n_threads(1);
            p.set_token_timestamps(true);
            p.set_split_on_word(true);
            p.set_single_segment(true);
            p.set_suppress_blank(true);
            p.set_suppress_nst(true);

            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p
        };

        self.state.full(params, &audio[..]).unwrap();
        let num_segments = self.state.full_n_segments().unwrap();

        let mut segments = Vec::new();
        for i in 0..num_segments {
            let text = self.state.full_get_segment_text(i).unwrap();
            let (start, end) = (
                self.state.full_get_segment_t0(i).unwrap(),
                self.state.full_get_segment_t1(i).unwrap(),
            );
            let confidence = self.calculate_segment_confidence(i);

            segments.push(Segment {
                text,
                start: start as f32 / 1000.0,
                end: end as f32 / 1000.0,
                confidence,
            });
        }

        self.prompt = segments
            .iter()
            .map(|s| s.text())
            .collect::<Vec<&str>>()
            .join(" ");

        segments
    }

    // https://github.com/ggml-org/whisper.cpp/pull/971/files#diff-2d3599a9fad195f2c3c60bd06691bc1815325b3560b5feda41a91fa71194e805R310-R327
    fn calculate_segment_confidence(&self, segment_idx: i32) -> f32 {
        let n_tokens = self.state.full_n_tokens(segment_idx).unwrap_or(0);
        if n_tokens == 0 {
            return 0.0;
        }

        let mut total_confidence = 0.0;
        let mut valid_tokens = 0;

        for j in 0..n_tokens {
            let token_id = match self.state.full_get_token_id(segment_idx, j) {
                Ok(id) => id,
                Err(_) => continue,
            };

            if token_id >= self.eot {
                continue;
            }

            let token_p = self
                .state
                .full_get_token_prob(segment_idx, j)
                .unwrap_or(0.0);

            let token_confidence = token_p.powi(3);

            total_confidence += token_confidence;
            valid_tokens += 1;
        }

        if valid_tokens == 0 {
            return 0.0;
        }

        total_confidence / valid_tokens as f32
    }
}

// https://github.com/floneum/floneum/blob/52967ae/models/rwhisper/src/lib.rs#L116
#[derive(Debug)]
pub struct Segment {
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub confidence: f32,
}

impl Segment {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn start(&self) -> f32 {
        self.start
    }

    pub fn end(&self) -> f32 {
        self.end
    }

    pub fn duration(&self) -> f32 {
        self.end - self.start
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[test]
    fn test_whisper() {
        let mut whisper = Whisper::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model.bin"));

        let audio: Vec<f32> = hypr_data::english_1::AUDIO
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();

        let segments = whisper.transcribe(&audio);
        assert!(segments.len() > 0);
    }

    #[tokio::test]
    async fn test_whisper_with_llama() {
        let llama_path = dirs::data_dir()
            .unwrap()
            .join("com.hyprnote.dev")
            .join("llm.gguf");

        let llama = hypr_llama::Llama::new(llama_path).unwrap();
        let mut whisper = Whisper::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model.bin"));

        let request = hypr_llama::LlamaRequest::new(vec![hypr_llama::LlamaChatMessage::new(
            "user".into(),
            "Generate a json array of 1 random objects, about animals".into(),
        )
        .unwrap()]);

        let response: String = llama.generate_stream(request).unwrap().collect().await;
        assert!(response.len() > 4);

        let audio: Vec<f32> = hypr_data::english_1::AUDIO
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .take(16000 * 30)
            .collect();

        let segments = whisper.transcribe(&audio);
        assert!(segments.len() > 0);
    }
}
