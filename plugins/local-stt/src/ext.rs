use tauri::{Manager, Runtime};

pub trait LocalSttPluginExt<R: Runtime> {
    fn load_model(
        &self,
        on_progress: tauri::ipc::Channel<u8>,
    ) -> impl std::future::Future<Output = Result<(), String>>;
    fn unload_model(&self) -> Result<(), String>;
}

impl<R: Runtime, T: Manager<R>> LocalSttPluginExt<R> for T {
    async fn load_model(&self, on_progress: tauri::ipc::Channel<u8>) -> Result<(), String> {
        let data_dir = self.path().app_data_dir().unwrap();

        let model = crate::model::model_builder(data_dir)
            .with_source(rwhisper::WhisperSource::QuantizedLargeV3Turbo)
            .build_with_loading_handler(crate::model::make_progress_handler(on_progress))
            .await
            .map_err(|e| e.to_string())?;

        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.model = Some(model);
        Ok(())
    }

    fn unload_model(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.model.take();
        Ok(())
    }
}
