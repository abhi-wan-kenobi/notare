pub fn model_builder(data_dir: std::path::PathBuf) -> rwhisper::WhisperBuilder {
    let cache = kalosm_common::Cache::new(data_dir)
        .with_huggingface_token(Some("***REMOVED-UPSTREAM-HF-TOKEN***".to_string()));

    rwhisper::WhisperBuilder::default().with_cache(cache)
}

pub fn make_progress_handler(
    on_progress: tauri::ipc::Channel<u8>,
) -> impl FnMut(rwhisper::ModelLoadingProgress) {
    let mut last_v: Option<u8> = None;

    move |progress| {
        if let rwhisper::ModelLoadingProgress::Downloading { progress, .. } = progress {
            let v = ((progress.progress as f32 / progress.size as f32) * 100.0) as u8;

            if last_v != Some(v) && on_progress.send(v).is_ok() {
                last_v = Some(v);
            }
        }
    }
}
