use owhisper_interface::Word2;

use crate::engine::{EngineError, SttEngine};

pub fn process_recorded<E: SttEngine>(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
) -> Result<Vec<Word2>, crate::Error> {
    let engine =
        E::load(model_path.as_ref()).map_err(|e| EngineError::new(e.to_string()))?;
    super::batch::transcribe_recorded_file(&engine, model_path.as_ref(), audio_path.as_ref())
}
