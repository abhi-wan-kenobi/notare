use std::future::Future;
use std::path::PathBuf;

use tauri::{ipc::Channel, Manager, Runtime};

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub dir: PathBuf,
    pub source: rwhisper::WhisperSource,
}

pub trait LocalSttPluginExt<R: Runtime> {
    fn api_base(&self) -> impl Future<Output = Option<String>>;
    fn is_model_downloaded(&self) -> impl Future<Output = Result<bool, crate::Error>>;
    fn is_server_running(&self) -> impl Future<Output = bool>;
    fn start_server(&self) -> impl Future<Output = Result<(), crate::Error>>;
    fn stop_server(&self) -> impl Future<Output = Result<(), crate::Error>>;
    fn download_config(&self, path: PathBuf) -> impl Future<Output = Result<(), String>>;
    fn download_tokenizer(&self, path: PathBuf) -> impl Future<Output = Result<(), String>>;
    fn download_model(
        &self,
        path: PathBuf,
        channel: Channel<u8>,
    ) -> impl Future<Output = Result<(), String>>;
}

impl<R: Runtime, T: Manager<R>> LocalSttPluginExt<R> for T {
    #[tracing::instrument(skip_all)]
    async fn api_base(&self) -> Option<String> {
        let state = self.state::<crate::SharedState>();
        let s = state.lock().await;

        s.api_base.clone()
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloaded(&self) -> Result<bool, crate::Error> {
        let base_path = self
            .path()
            .app_data_dir()
            .unwrap()
            .join("Demonthos/candle-quantized-whisper-large-v3-turbo/main/");

        let model_path = base_path.join("model.gguf");
        let config_path = base_path.join("config.json");
        let tokenizer_path = base_path.join("tokenizer.json");

        if [&model_path, &config_path, &tokenizer_path]
            .iter()
            .any(|p| !p.exists())
        {
            return Ok(false);
        }

        for (path, expected) in [
            (model_path, 800664009),
            (config_path, 472563957),
            (tokenizer_path, 1395948910),
        ] {
            let actual = hypr_file::calculate_file_checksum(path)?;

            if actual != expected {
                return Ok(false);
            }
        }

        Ok(true)
    }

    #[tracing::instrument(skip_all)]
    async fn is_server_running(&self) -> bool {
        let state = self.state::<crate::SharedState>();
        let s = state.lock().await;

        s.server.is_some()
    }

    #[tracing::instrument(skip_all)]
    async fn start_server(&self) -> Result<(), crate::Error> {
        let cache_dir = self.path().app_data_dir()?;

        let server_state = crate::ServerStateBuilder::default()
            .model_cache_dir(cache_dir)
            .model_type(rwhisper::WhisperSource::QuantizedLargeV3Turbo)
            .build();

        let server = crate::run_server(server_state).await?;

        {
            let state = self.state::<crate::SharedState>();
            let mut s = state.lock().await;
            s.api_base = Some(format!("http://{}", &server.addr));
            s.server = Some(server);
        }

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

    #[tracing::instrument(skip_all)]
    async fn download_config(&self, path: PathBuf) -> Result<(), String> {
        let url = "https://pub-8987485129c64debb63bff7f35a2e5fd.r2.dev/v0/Demonthos/candle-quantized-whisper-large-v3-turbo/main/config.json";

        tokio::spawn(async move {
            let callback = |_: u64, _: u64| {};

            if let Err(e) = hypr_file::download_file_with_callback(url, path, callback).await {
                tracing::error!("Failed to download config: {}", e);
            }
        });

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn download_tokenizer(&self, path: PathBuf) -> Result<(), String> {
        let url = "https://pub-8987485129c64debb63bff7f35a2e5fd.r2.dev/v0/Demonthos/candle-quantized-whisper-large-v3-turbo/main/tokenizer.json";

        tokio::spawn(async move {
            let callback = |_: u64, _: u64| {};

            if let Err(e) = hypr_file::download_file_with_callback(url, path, callback).await {
                tracing::error!("Failed to download tokenizer: {}", e);
            }
        });

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn download_model(&self, path: PathBuf, channel: Channel<u8>) -> Result<(), String> {
        let url = "https://pub-8987485129c64debb63bff7f35a2e5fd.r2.dev/v0/Demonthos/candle-quantized-whisper-large-v3-turbo/main/model.gguf";

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
}
