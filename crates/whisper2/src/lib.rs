// https://github.com/tazz4843/whisper-rs/blob/master/examples/audio_transcription.rs

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct Whisper {
    state: WhisperState,
}

impl Whisper {
    pub fn new(model_path: &str) -> Self {
        let context_param = {
            let mut p = WhisperContextParameters::default();
            p.dtw_parameters.mode = whisper_rs::DtwMode::None;
            p
        };

        let ctx = WhisperContext::new_with_params(model_path, context_param).unwrap();
        let state = ctx.create_state().unwrap();

        Self { state }
    }

    pub fn transcribe(&mut self, audio: &[f32]) {
        let params = {
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_translate(false);
            p.set_language(Some("en"));
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_token_timestamps(true);
            p.set_initial_prompt("");
            p.set_split_on_word(true);
            p.set_single_segment(false);
            p.set_suppress_blank(true);
            p.set_suppress_nst(true);
            p
        };

        self.state.full(params, &audio[..]).unwrap();
        let num_segments = self.state.full_n_segments().unwrap();

        for i in 0..num_segments {
            let text = self.state.full_get_segment_text(i).unwrap();
            let (start, end) = (
                self.state.full_get_segment_t0(i).unwrap(),
                self.state.full_get_segment_t1(i).unwrap(),
            );
            println!("[{} - {}]: {}", start, end, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // https://github.com/utilityai/llama-cpp-rs/issues/484
    #[test]
    fn test_whisper() {
        let mut whisper = Whisper::new(concat!(env!("CARGO_MANIFEST_DIR"), "/model.bin"));

        let audio: Vec<f32> = hypr_data::english_1::AUDIO
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .take(16000 * 30)
            .collect();

        whisper.transcribe(&audio);
    }
}
