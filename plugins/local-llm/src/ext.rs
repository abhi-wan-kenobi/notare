use std::{collections::HashMap, future::Future, path::PathBuf, sync::Arc};
use tauri::{Manager, Runtime, ipc::Channel};

use hypr_model_downloader::{DownloadableModel, ModelDownloadManager, ModelDownloaderRuntime};

struct TauriModelRuntime<R: Runtime> {
    app_handle: tauri::AppHandle<R>,
    channels: Arc<std::sync::Mutex<HashMap<String, Channel<i8>>>>,
}

impl<R: Runtime> ModelDownloaderRuntime<crate::SupportedModel> for TauriModelRuntime<R> {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        Ok(models_base(&self.app_handle))
    }

    fn emit_progress(
        &self,
        model: &crate::SupportedModel,
        status: hypr_model_downloader::DownloadStatus,
    ) {
        use hypr_model_downloader::DownloadStatus;

        let progress: i8 = match &status {
            DownloadStatus::Downloading(p) => *p as i8,
            DownloadStatus::Completed => 100,
            DownloadStatus::Failed(_) => -1,
        };

        let key = model.download_key();
        let mut guard = self.channels.lock().unwrap();

        let Some(channel) = guard.get(&key) else {
            return;
        };

        let send_result = channel.send(progress);
        let is_terminal = matches!(
            status,
            DownloadStatus::Completed | DownloadStatus::Failed(_)
        );
        if send_result.is_err() || is_terminal {
            guard.remove(&key);
        }
    }
}

pub fn create_model_downloader<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    channels: Arc<std::sync::Mutex<HashMap<String, Channel<i8>>>>,
) -> ModelDownloadManager<crate::SupportedModel> {
    let runtime = Arc::new(TauriModelRuntime {
        app_handle: app_handle.clone(),
        channels,
    });
    ModelDownloadManager::new(runtime)
}

fn models_base<R: Runtime, T: Manager<R>>(manager: &T) -> PathBuf {
    use tauri_plugin_settings::SettingsPluginExt;

    manager
        .settings()
        .global_base()
        .map(|base| base.join("models").into_std_path_buf())
        .unwrap_or_else(|_| dirs::data_dir().unwrap_or_default().join("models"))
}

pub trait LocalLlmPluginExt<R: Runtime> {
    fn models_dir(&self) -> PathBuf;

    fn list_downloaded_model(
        &self,
    ) -> impl Future<Output = Result<Vec<crate::SupportedModel>, crate::Error>>;

    fn list_custom_models(
        &self,
    ) -> impl Future<Output = Result<Vec<crate::CustomModelInfo>, crate::Error>>;

    fn download_model(
        &self,
        model: crate::SupportedModel,
        channel: Channel<i8>,
    ) -> impl Future<Output = Result<(), crate::Error>>;
    fn cancel_download(
        &self,
        model: crate::SupportedModel,
    ) -> impl Future<Output = Result<bool, crate::Error>>;
    fn delete_model(
        &self,
        model: &crate::SupportedModel,
    ) -> impl Future<Output = Result<(), crate::Error>>;
    fn is_model_downloading(&self, model: &crate::SupportedModel) -> impl Future<Output = bool>;
    fn is_model_downloaded(
        &self,
        model: &crate::SupportedModel,
    ) -> impl Future<Output = Result<bool, crate::Error>>;
    fn server_url(&self) -> impl Future<Output = Result<Option<String>, crate::Error>>;
}

impl<R: Runtime, T: Manager<R>> LocalLlmPluginExt<R> for T {
    fn models_dir(&self) -> PathBuf {
        hypr_local_llm_core::llm_models_dir(&models_base(self))
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloading(&self, model: &crate::SupportedModel) -> bool {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        downloader.is_downloading(model).await
    }

    #[tracing::instrument(skip_all)]
    async fn is_model_downloaded(
        &self,
        model: &crate::SupportedModel,
    ) -> Result<bool, crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };
        Ok(downloader.is_downloaded(model).await?)
    }

    #[tracing::instrument(skip_all)]
    async fn server_url(&self) -> Result<Option<String>, crate::Error> {
        let state = self.state::<crate::SharedState>();
        let guard = state.lock().await;

        Ok(guard.server.as_ref().map(|server| server.url().to_string()))
    }

    #[tracing::instrument(skip_all)]
    async fn download_model(
        &self,
        model: crate::SupportedModel,
        channel: Channel<i8>,
    ) -> Result<(), crate::Error> {
        let download_model = model;
        let key = download_model.download_key();

        let (downloader, channels) = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            (
                guard.model_downloader.clone(),
                guard.download_channels.clone(),
            )
        };

        downloader.cancel_download(&download_model).await?;

        {
            let mut guard = channels.lock().unwrap();
            if let Some(existing) = guard.insert(key.clone(), channel) {
                let _ = existing.send(-1);
            }
        }

        if let Err(e) = downloader.download(&download_model).await {
            let mut guard = channels.lock().unwrap();
            if let Some(channel) = guard.remove(&key) {
                let _ = channel.send(-1);
            }
            return Err(e.into());
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn cancel_download(&self, model: crate::SupportedModel) -> Result<bool, crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        Ok(downloader.cancel_download(&model).await?)
    }

    #[tracing::instrument(skip_all)]
    async fn delete_model(&self, model: &crate::SupportedModel) -> Result<(), crate::Error> {
        let downloader = {
            let state = self.state::<crate::SharedState>();
            let guard = state.lock().await;
            guard.model_downloader.clone()
        };

        downloader.delete(model).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn list_downloaded_model(&self) -> Result<Vec<crate::SupportedModel>, crate::Error> {
        Ok(hypr_local_llm_core::list_downloaded_models(
            &self.models_dir(),
        )?)
    }

    #[tracing::instrument(skip_all)]
    async fn list_custom_models(&self) -> Result<Vec<crate::CustomModelInfo>, crate::Error> {
        Ok(hypr_local_llm_core::list_custom_models()?)
    }
}
