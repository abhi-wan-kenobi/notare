use std::future::Future;

use tauri::{ipc::Channel, Manager, Runtime};

pub trait LocalLlmPluginExt<R: Runtime> {
    fn api_base(&self) -> impl Future<Output = Option<String>>;
    fn is_model_downloaded(&self) -> impl Future<Output = Result<bool, crate::Error>>;
    fn is_server_running(&self) -> impl Future<Output = bool>;
    fn download_model(&self, channel: Channel<u8>) -> impl Future<Output = Result<(), String>>;
    fn start_server(&self) -> impl Future<Output = Result<(), crate::Error>>;
    fn stop_server(&self) -> impl Future<Output = Result<(), crate::Error>>;
}

impl<R: Runtime, T: Manager<R>> LocalLlmPluginExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn api_base(&self) -> Option<String> {
        let state = self.state::<crate::SharedState>();
        let s = state.lock().await;
        s.api_base.clone()
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloaded(&self) -> Result<bool, crate::Error> {
        let path = self.path().app_data_dir().unwrap().join("llm.gguf");

        if !path.exists() {
            return Ok(false);
        }

        let checksum = hypr_file::calculate_file_checksum(&path)?;
        Ok(checksum == 2831308098)
    }

    #[tracing::instrument(skip_all)]
    async fn is_server_running(&self) -> bool {
        let state = self.state::<crate::SharedState>();
        let s = state.lock().await;
        s.server.is_some()
    }

    #[tracing::instrument(skip_all)]
    async fn download_model(&self, channel: Channel<u8>) -> Result<(), String> {
        let path = self.path().app_data_dir().unwrap().join("llm.gguf");
        let url = "https://pub-8987485129c64debb63bff7f35a2e5fd.r2.dev/v0/lmstudio-community/Llama-3.2-3B-Instruct-GGUF/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf";

        tokio::spawn(async move {
            let callback = |downloaded: u64, total_size: u64| {
                let percent = (downloaded as f64 / total_size as f64) * 100.0;

                let _ = channel.send(percent as u8);
            };

            if let Err(e) = hypr_file::download_file_with_callback(url, path, callback).await {
                tracing::error!("Failed to download model: {}", e);
            }
        });

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn start_server(&self) -> Result<(), crate::Error> {
        let state = self.state::<crate::SharedState>();

        let model_manager = {
            let s = state.lock().await;
            crate::ModelManager::new(s.model_path.clone())
        };

        let server = crate::server::run_server(model_manager).await?;

        let mut s = state.lock().await;
        s.api_base = Some(format!("http://{}", &server.addr));
        s.server = Some(server);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn stop_server(&self) -> Result<(), crate::Error> {
        let state = self.state::<crate::SharedState>();
        let mut s = state.lock().await;

        if let Some(server) = s.server.take() {
            let _ = server.shutdown.send(());
        }
        Ok(())
    }
}
