use std::future::Future;
use tauri::{ipc::Channel, Manager, Runtime};

pub trait LocalLlmPluginExt<R: Runtime> {
    fn load_model(&self, on_progress: Channel<u8>) -> impl Future<Output = Result<(), String>>;
    fn unload_model(&self) -> impl Future<Output = Result<(), String>>;
    fn stop_server(&self) -> impl Future<Output = Result<(), String>>;
}

impl<R: Runtime, T: Manager<R>> LocalLlmPluginExt<R> for T {
    async fn load_model(&self, on_progress: Channel<u8>) -> Result<(), String> {
        let data_dir = self.path().app_data_dir().unwrap();
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;

        let model =
            crate::model::model_builder(data_dir, kalosm_llama::LlamaSource::llama_3_2_3b_chat())
                .build_with_loading_handler(crate::model::make_progress_handler(on_progress))
                .await
                .map_err(|e| e.to_string())?;

        s.model = Some(model);
        Ok(())
    }

    async fn unload_model(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;
        s.model.take();
        Ok(())
    }

    async fn stop_server(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;
        s.server.take();
        Ok(())
    }
}
