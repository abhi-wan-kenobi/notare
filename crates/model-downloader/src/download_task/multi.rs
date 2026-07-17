//! Multi-part download task: sequentially downloads every part of a
//! multi-file model into `.part-<generation>` temps under one cancellation
//! token, verifies each part (size + CRC32), then renames all parts into
//! place and finalizes. Aggregate progress is weighted by expected part
//! sizes and flows through the existing `DownloadStatus::Downloading`
//! events, so the frontend works unchanged.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use hypr_download_interface::DownloadProgress;

use crate::download_paths::generation_download_path;
use crate::download_task_progress::make_progress_callback;
use crate::downloads_registry::DownloadsRegistry;
use crate::model::{DownloadPart, DownloadableModel};
use crate::runtime::{DownloadStatus, ModelDownloaderRuntime};

pub(crate) struct MultiDownloadTaskParams<M: DownloadableModel> {
    pub(crate) runtime: Arc<dyn ModelDownloaderRuntime<M>>,
    pub(crate) registry: DownloadsRegistry,
    pub(crate) model: M,
    pub(crate) parts: Vec<DownloadPart>,
    /// Final destination directory the parts are installed into.
    pub(crate) destination_dir: PathBuf,
    pub(crate) models_base: PathBuf,
    pub(crate) key: String,
    pub(crate) generation: u64,
    pub(crate) cancellation_token: CancellationToken,
}

struct PlannedPart {
    part: DownloadPart,
    temp_path: PathBuf,
    final_path: PathBuf,
}

pub(crate) fn spawn_multi_download_task<M: DownloadableModel>(
    params: MultiDownloadTaskParams<M>,
    start_rx: oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let planned: Vec<PlannedPart> = params
            .parts
            .iter()
            .map(|part| {
                let final_path = params.destination_dir.join(&part.relative_path);
                PlannedPart {
                    part: part.clone(),
                    temp_path: generation_download_path(&final_path, params.generation),
                    final_path,
                }
            })
            .collect();

        if start_rx.await.is_err() {
            cleanup(&params, &planned).await;
            return;
        }

        match run(&params, &planned).await {
            Ok(()) => {
                params
                    .runtime
                    .emit_progress(&params.model, DownloadStatus::Completed);
                params
                    .registry
                    .remove_if_generation_matches(&params.key, params.generation)
                    .await;
            }
            Err(reason) => {
                if let Some(reason) = reason {
                    params
                        .runtime
                        .emit_progress(&params.model, DownloadStatus::Failed(reason));
                }
                cleanup(&params, &planned).await;
            }
        }
    })
}

/// `Err(None)` = cancelled (no Failed event; `cancel_download` emits its own),
/// `Err(Some(reason))` = failed.
async fn run<M: DownloadableModel>(
    params: &MultiDownloadTaskParams<M>,
    planned: &[PlannedPart],
) -> Result<(), Option<String>> {
    let total_bytes: u64 = planned
        .iter()
        .map(|p| p.part.expected_size.unwrap_or(0))
        .sum();

    let emit = make_progress_callback(params.runtime.clone(), params.model.clone());
    emit(DownloadProgress::Started);

    let mut completed_bytes: u64 = 0;

    for planned_part in planned {
        let part = &planned_part.part;
        let offset = completed_bytes;

        let part_progress = |progress: DownloadProgress| match progress {
            // Started/Finished are whole-download events; synthesized once
            // for the aggregate stream instead.
            DownloadProgress::Started | DownloadProgress::Finished => {}
            DownloadProgress::Progress(downloaded, _part_total) => {
                if total_bytes > 0 {
                    emit(DownloadProgress::Progress(
                        (offset + downloaded).min(total_bytes),
                        total_bytes,
                    ));
                }
            }
        };

        hypr_file::download_file_parallel_cancellable(
            &part.url,
            &planned_part.temp_path,
            part_progress,
            Some(params.cancellation_token.clone()),
        )
        .await
        .map_err(|error| download_failure_reason(&part.relative_path, &error))?;

        verify_part(planned_part).await?;

        completed_bytes += part.expected_size.unwrap_or(0);
        if total_bytes > 0 {
            emit(DownloadProgress::Progress(
                completed_bytes.min(total_bytes),
                total_bytes,
            ));
        }
    }

    // All parts verified: move them into place, then finalize.
    for planned_part in planned {
        promote(planned_part).await.map_err(|error| {
            tracing::error!(error = %error, "multi_part_promote_error");
            Some(format!("Failed to move model file: {}", error))
        })?;
    }

    emit(DownloadProgress::Finished);

    let model = params.model.clone();
    let destination = params.destination_dir.clone();
    let models_base = params.models_base.clone();
    let finalize_result =
        tokio::task::spawn_blocking(move || model.finalize_download(&destination, &models_base))
            .await;

    match finalize_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            tracing::error!(error = %error, "multi_part_finalize_error");
            Err(Some(format!("Failed to finalize model: {}", error)))
        }
        Err(error) => {
            tracing::error!(error = %error, "multi_part_finalize_join_error");
            Err(Some(format!("Finalization interrupted: {}", error)))
        }
    }
}

fn download_failure_reason(relative_path: &str, error: &hypr_file::Error) -> Option<String> {
    if matches!(error, hypr_file::Error::Cancelled) {
        return None;
    }

    tracing::error!(part = relative_path, error = %error, "multi_part_download_error");

    let reason = match error {
        hypr_file::Error::ReqwestError(e) => {
            if e.is_timeout() {
                "Download timed out. Please check your internet connection and try again."
                    .to_string()
            } else if e.is_connect() {
                "Could not connect to the download server. Please check your internet connection."
                    .to_string()
            } else {
                format!("Network error: {}", e)
            }
        }
        hypr_file::Error::FileIOError(e) => format!("File system error: {}", e),
        hypr_file::Error::Cancelled => unreachable!(),
        hypr_file::Error::OtherError(msg) => msg.clone(),
    };
    Some(reason)
}

async fn verify_part(planned_part: &PlannedPart) -> Result<(), Option<String>> {
    let part = &planned_part.part;

    if let Some(expected_size) = part.expected_size {
        let actual_size = tokio::fs::metadata(&planned_part.temp_path)
            .await
            .map(|meta| meta.len())
            .map_err(|e| Some(format!("Failed to verify download: {}", e)))?;

        if actual_size != expected_size {
            tracing::error!(
                part = %part.relative_path,
                expected_size,
                actual_size,
                "multi_part_size_mismatch"
            );
            return Err(Some(
                "Downloaded file is corrupted (size mismatch). Please try again.".to_string(),
            ));
        }
    }

    if let Some(expected_checksum) = part.checksum {
        let temp_path = planned_part.temp_path.clone();
        let checksum_result =
            tokio::task::spawn_blocking(move || hypr_file::calculate_file_checksum(temp_path))
                .await;

        match checksum_result {
            Ok(Ok(actual)) if actual == expected_checksum => {}
            Ok(Ok(actual)) => {
                tracing::error!(
                    part = %part.relative_path,
                    actual_checksum = actual,
                    expected_checksum,
                    "multi_part_checksum_mismatch"
                );
                return Err(Some(
                    "Downloaded file is corrupted (checksum mismatch). Please try again."
                        .to_string(),
                ));
            }
            Ok(Err(error)) => {
                tracing::error!(part = %part.relative_path, error = %error, "multi_part_checksum_error");
                return Err(Some(format!("Failed to verify download: {}", error)));
            }
            Err(error) => {
                tracing::error!(part = %part.relative_path, error = %error, "multi_part_checksum_join_error");
                return Err(Some(format!("Verification interrupted: {}", error)));
            }
        }
    }

    Ok(())
}

async fn promote(planned_part: &PlannedPart) -> Result<(), std::io::Error> {
    if let Some(parent) = planned_part.final_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    match tokio::fs::rename(&planned_part.temp_path, &planned_part.final_path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = tokio::fs::remove_file(&planned_part.final_path).await;
            tokio::fs::rename(&planned_part.temp_path, &planned_part.final_path).await
        }
        Err(e) => Err(e),
    }
}

async fn cleanup<M: DownloadableModel>(
    params: &MultiDownloadTaskParams<M>,
    planned: &[PlannedPart],
) {
    for planned_part in planned {
        let _ = tokio::fs::remove_file(&planned_part.temp_path).await;
    }
    params
        .registry
        .remove_if_generation_matches(&params.key, params.generation)
        .await;
}
