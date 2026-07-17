use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::fs;

use crate::Error;
use crate::download_paths::generation_download_path;
use crate::download_task::{DownloadTaskParams, spawn_download_task};
use crate::downloads_registry::{DownloadEntry, DownloadsRegistry};
use crate::integrity::{self, ModelIntegrity};
use crate::model::DownloadableModel;
use crate::runtime::{DownloadStatus, ModelDownloaderRuntime};
use crate::task_join::wait_for_task_exit;

pub struct ModelDownloadManager<M: DownloadableModel> {
    runtime: Arc<dyn ModelDownloaderRuntime<M>>,
    downloads: DownloadsRegistry,
    next_generation: Arc<AtomicU64>,
}

impl<M: DownloadableModel> Clone for ModelDownloadManager<M> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            downloads: self.downloads.clone(),
            next_generation: self.next_generation.clone(),
        }
    }
}

impl<M: DownloadableModel> ModelDownloadManager<M> {
    const TASK_JOIN_WARN_AFTER: Duration = Duration::from_secs(5);

    pub fn new(runtime: Arc<dyn ModelDownloaderRuntime<M>>) -> Self {
        Self {
            runtime,
            downloads: DownloadsRegistry::new(),
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn model_path(&self, model: &M) -> Result<PathBuf, Error> {
        let models_base = self.runtime.models_base()?;
        Ok(model.download_destination(&models_base))
    }

    pub async fn is_downloaded(&self, model: &M) -> Result<bool, Error> {
        let models_base = self.runtime.models_base()?;
        let model_clone = model.clone();
        tokio::task::spawn_blocking(move || model_clone.is_downloaded(&models_base))
            .await
            .map_err(|e| Error::OperationFailed(e.to_string()))?
    }

    pub async fn is_downloading(&self, model: &M) -> bool {
        self.downloads.contains(&model.download_key()).await
    }

    pub async fn verify_integrity(&self, model: &M) -> Result<ModelIntegrity, Error> {
        let models_base = self.runtime.models_base()?;
        let model_clone = model.clone();
        tokio::task::spawn_blocking(move || integrity::verify_model(&model_clone, &models_base))
            .await
            .map_err(|e| Error::OperationFailed(e.to_string()))?
    }

    /// Startup reconciliation: verify each model's declared state against the
    /// filesystem. Corrupt files are quarantined (renamed to `*.corrupt`) so
    /// every status query reports them as not installed, and a Failed
    /// progress event is emitted so the UI surfaces a re-download action.
    /// Models with a download in flight are skipped.
    pub async fn reconcile(&self, models: &[M]) -> Vec<(M, ModelIntegrity)> {
        let mut results = Vec::with_capacity(models.len());
        for model in models {
            if self.is_downloading(model).await {
                continue;
            }
            match self.verify_integrity(model).await {
                Ok(ModelIntegrity::Corrupt(reason)) => {
                    tracing::warn!(
                        model = %model.download_key(),
                        %reason,
                        "model_integrity_corrupt"
                    );
                    if let Err(e) = self.quarantine(model).await {
                        tracing::error!(
                            model = %model.download_key(),
                            error = %e,
                            "model_quarantine_failed"
                        );
                    }
                    self.runtime.emit_progress(
                        model,
                        DownloadStatus::Failed(format!("integrity check failed: {reason}")),
                    );
                    results.push((model.clone(), ModelIntegrity::Corrupt(reason)));
                }
                Ok(state) => results.push((model.clone(), state)),
                Err(e) => {
                    tracing::warn!(
                        model = %model.download_key(),
                        error = %e,
                        "model_integrity_check_errored"
                    );
                }
            }
        }
        results
    }

    async fn quarantine(&self, model: &M) -> Result<(), Error> {
        let models_base = self.runtime.models_base()?;
        let model_clone = model.clone();
        tokio::task::spawn_blocking(move || {
            let destination = model_clone.download_destination(&models_base);
            integrity::remove_stamp(&destination);
            let mut name = destination
                .file_name()
                .unwrap_or_default()
                .to_os_string();
            name.push(".corrupt");
            let quarantine_path = destination.with_file_name(name);
            if destination.is_file() {
                std::fs::rename(&destination, &quarantine_path)
                    .map_err(|e| Error::OperationFailed(e.to_string()))?;
            } else if destination.is_dir() {
                // Multi-part models install as a directory; quarantine the
                // whole directory (stamps live inside and move with it).
                let _ = std::fs::remove_dir_all(&quarantine_path);
                std::fs::rename(&destination, &quarantine_path)
                    .map_err(|e| Error::OperationFailed(e.to_string()))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::OperationFailed(e.to_string()))?
    }

    pub async fn download(&self, model: &M) -> Result<(), Error> {
        if let Some(parts) = model.download_parts() {
            return self.download_multi(model, parts).await;
        }

        let key = model.download_key();
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);

        let url = model
            .download_url()
            .ok_or_else(|| Error::NoDownloadUrl(model.download_key()))?;

        let models_base = self.runtime.models_base()?;
        let final_destination = model.download_destination(&models_base);
        let destination = generation_download_path(&final_destination, generation);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        let (start_tx, start_rx) = tokio::sync::oneshot::channel::<()>();

        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let task = spawn_download_task(
            DownloadTaskParams {
                runtime: self.runtime.clone(),
                registry: self.downloads.clone(),
                model: model.clone(),
                url,
                destination: destination.clone(),
                final_destination: final_destination.clone(),
                models_base: models_base.clone(),
                key: key.clone(),
                generation,
                cancellation_token: cancellation_token.clone(),
            },
            start_rx,
        );

        let existing = self
            .downloads
            .insert(
                key,
                DownloadEntry {
                    task,
                    token: cancellation_token,
                    generation,
                    download_path: destination,
                },
            )
            .await;

        if let Some(entry) = existing {
            entry.token.cancel();
            wait_for_task_exit(
                entry.task,
                Self::TASK_JOIN_WARN_AFTER,
                "replace_existing_download",
            )
            .await;
        }

        let _ = start_tx.send(());

        Ok(())
    }

    async fn download_multi(
        &self,
        model: &M,
        parts: Vec<crate::model::DownloadPart>,
    ) -> Result<(), Error> {
        use crate::download_task::{MultiDownloadTaskParams, spawn_multi_download_task};

        let key = model.download_key();
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);

        let models_base = self.runtime.models_base()?;
        let destination_dir = model.download_destination(&models_base);
        fs::create_dir_all(&destination_dir).await?;

        let (start_tx, start_rx) = tokio::sync::oneshot::channel::<()>();

        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let task = spawn_multi_download_task(
            MultiDownloadTaskParams {
                runtime: self.runtime.clone(),
                registry: self.downloads.clone(),
                model: model.clone(),
                parts,
                destination_dir: destination_dir.clone(),
                models_base,
                key: key.clone(),
                generation,
                cancellation_token: cancellation_token.clone(),
            },
            start_rx,
        );

        let existing = self
            .downloads
            .insert(
                key,
                DownloadEntry {
                    task,
                    token: cancellation_token,
                    generation,
                    // The task cleans its own `.part-*` temps on
                    // cancel/failure; removing this directory path here is a
                    // harmless no-op in `cancel_download`.
                    download_path: destination_dir,
                },
            )
            .await;

        if let Some(entry) = existing {
            entry.token.cancel();
            wait_for_task_exit(
                entry.task,
                Self::TASK_JOIN_WARN_AFTER,
                "replace_existing_download",
            )
            .await;
        }

        let _ = start_tx.send(());

        Ok(())
    }

    pub async fn cancel_download(&self, model: &M) -> Result<bool, Error> {
        let key = model.download_key();

        let existing = self.downloads.remove(&key).await;

        if let Some(entry) = existing {
            entry.token.cancel();
            wait_for_task_exit(entry.task, Self::TASK_JOIN_WARN_AFTER, "cancel_download").await;
            self.runtime.emit_progress(
                model,
                crate::runtime::DownloadStatus::Failed("Download cancelled".to_string()),
            );
            let _ = fs::remove_file(entry.download_path).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn delete(&self, model: &M) -> Result<(), Error> {
        if !self.is_downloaded(model).await? {
            return Err(Error::ModelNotDownloaded(model.download_key()));
        }

        let models_base = self.runtime.models_base()?;
        let model_clone = model.clone();
        tokio::task::spawn_blocking(move || {
            let result = model_clone.delete_downloaded(&models_base);
            if result.is_ok() {
                integrity::remove_stamp(&model_clone.download_destination(&models_base));
            }
            result
        })
        .await
        .map_err(|e| Error::OperationFailed(e.to_string()))?
    }
}
