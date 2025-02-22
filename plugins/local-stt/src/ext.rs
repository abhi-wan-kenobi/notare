use std::future::Future;
use tauri::{ipc::Channel, Manager, Runtime};

pub trait LocalSttPluginExt<R: Runtime> {
    fn load_model(&self, on_progress: Channel<u8>) -> impl Future<Output = Result<(), String>>;
    fn unload_model(&self) -> impl Future<Output = Result<(), String>>;
    fn start_server(&self) -> impl Future<Output = Result<(), String>>;
    fn stop_server(&self) -> impl Future<Output = Result<(), String>>;
}

impl<R: Runtime, T: Manager<R>> LocalSttPluginExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn load_model(&self, on_progress: tauri::ipc::Channel<u8>) -> Result<(), String> {
        let data_dir = self.path().app_data_dir().unwrap();

        let model = crate::model::model_builder(data_dir)
            .with_source(rwhisper::WhisperSource::QuantizedLargeV3Turbo)
            .build_with_loading_handler(crate::model::make_progress_handler(on_progress))
            .await
            .map_err(|e| e.to_string())?;

        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;
        s.model = Some(model);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn unload_model(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;
        s.model.take();
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn start_server(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let server = crate::server::run_server(state.inner().clone())
            .await
            .map_err(|e| e.to_string())?;

        let mut s = state.lock().await;
        s.api_base = format!("http://{}", &server.addr);
        s.server = Some(server);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn stop_server(&self) -> Result<(), String> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;
        s.server.take();
        Ok(())
    }
}
