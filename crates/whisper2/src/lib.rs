// https://github.com/tazz4843/whisper-rs/blob/master/examples/audio_transcription.rs

use whisper_rs::WhisperContextParameters;

pub fn run() {
    let mut context_param = WhisperContextParameters::default();

    context_param.dtw_parameters.mode = whisper_rs::DtwMode::ModelPreset {
        model_preset: whisper_rs::DtwModelPreset::TinyEn,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // https://github.com/utilityai/llama-cpp-rs/issues/484
    #[test]
    fn test_whisper() {
        run();
    }
}
