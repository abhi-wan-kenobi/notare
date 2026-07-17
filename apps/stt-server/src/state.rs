use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use hypr_local_model::{LocalModel, WhisperModel};
use hypr_model_downloader::{DownloadStatus, ModelDownloadManager, ModelDownloaderRuntime, ModelIntegrity};
use hypr_transcribe_whisper_local::TranscribeService;
use tokio::sync::RwLock;

use crate::admin::models::CATALOG;
use crate::config::Config;

/// Runtime glue between `hypr_model_downloader::ModelDownloadManager` and
/// this server. Models live at `<model_dir>/stt/<file>` (the same layout the
/// desktop uses via `LocalModel::install_path`); download progress is kept
/// in an in-memory map rather than pushed anywhere, because a plain HTTP/JSON
/// server has no Tauri-event-style push channel — `GET /api/models` and
/// `GET /api/models/{id}/progress` poll it instead (see
/// `docs/stt-server-design.md`, Phase 2 addendum, for the rationale).
struct ServerModelRuntime {
    model_dir: PathBuf,
    progress: Arc<StdMutex<HashMap<String, DownloadStatus>>>,
}

impl ModelDownloaderRuntime<LocalModel> for ServerModelRuntime {
    fn models_base(&self) -> Result<PathBuf, hypr_model_downloader::Error> {
        Ok(self.model_dir.clone())
    }

    fn emit_progress(&self, model: &LocalModel, status: DownloadStatus) {
        // Only whisper models are ever downloaded through this server's
        // catalog (see `CATALOG`), but match exhaustively-by-variant rather
        // than assuming, in case that ever changes.
        if let LocalModel::Whisper(model) = model {
            self.progress
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(model.to_string(), status);
        }
    }
}

/// The currently-serving core: which catalog model id is active, and the
/// already-(re)built `TranscribeService` that `/v1/listen` dispatches to.
/// `POST /api/models/{id}/activate` swaps both fields atomically under one
/// write lock. The previous `TranscribeService` — and the
/// `hypr_model_manager::ModelManager` (+ loaded whisper context) it owns
/// internally — drops once no in-flight request still holds a clone of it,
/// which is how "unload" happens: there is no separate unload call to make,
/// Rust's ownership does it (see `docs/stt-server-design.md` Phase 2
/// addendum for why a fresh `TranscribeService` per activation, not a
/// server-wide `ModelManager<LoadedWhisper>`, is the swap unit here).
pub(crate) struct ActiveService {
    pub(crate) model: WhisperModel,
    pub(crate) core: TranscribeService,
}

/// Shared state for the `/api/*` admin handlers and the dynamic `/v1/listen`
/// dispatcher (`router::DynamicListen`).
pub struct AppState {
    pub config: Config,
    pub start_time: Instant,
    pub(crate) downloader: ModelDownloadManager<LocalModel>,
    pub(crate) progress: Arc<StdMutex<HashMap<String, DownloadStatus>>>,
    pub(crate) active: RwLock<ActiveService>,
}

/// `POST /api/models/{id}/activate` failure modes, mapped to HTTP status by
/// `admin::models::activate_model`.
#[derive(Debug)]
pub(crate) enum ActivateError {
    NotInstalled,
    Corrupt(String),
    IntegrityCheckFailed(String),
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let progress = Arc::new(StdMutex::new(HashMap::new()));
        let runtime = Arc::new(ServerModelRuntime {
            model_dir: config.model_dir.clone(),
            progress: progress.clone(),
        });
        let downloader = ModelDownloadManager::new(runtime);

        // Identical to Phase 1's `router::build_router` at startup: one
        // `TranscribeService` built for the configured default model, warmed
        // up in the background by its builder.
        let core = TranscribeService::builder()
            .model_path(config.model_path())
            .build();

        Self {
            active: RwLock::new(ActiveService {
                model: config.model.clone(),
                core,
            }),
            downloader,
            progress,
            start_time: Instant::now(),
            config,
        }
    }

    /// `<model_dir>/stt/<file name>` for any catalog model, matching
    /// `Config::model_path` for the configured default.
    pub(crate) fn model_path_for(&self, model: &WhisperModel) -> PathBuf {
        LocalModel::Whisper(model.clone()).install_path(&self.config.model_dir)
    }

    /// `POST /api/models/{id}/activate`: verify the model is installed
    /// (`Verified` or `PresentUnverified`) via
    /// `hypr_model_downloader::verify_model`, then rebuild the `/v1/listen`
    /// core against it — `TranscribeService::builder().build()` registers it
    /// with a fresh `hypr_model_manager::ModelManager` and kicks off the same
    /// background warmup load Phase 1 already relies on at startup, so
    /// "activate" both loads (eagerly, via warmup) and satisfies "lazy load
    /// on first request" (`ModelManager::get` loads on demand if the warmup
    /// hasn't finished or the model was since evicted for inactivity).
    pub(crate) async fn activate(&self, model: WhisperModel) -> Result<ModelIntegrity, ActivateError> {
        let local = LocalModel::Whisper(model.clone());
        let integrity = self
            .downloader
            .verify_integrity(&local)
            .await
            .map_err(|e| ActivateError::IntegrityCheckFailed(e.to_string()))?;

        match &integrity {
            ModelIntegrity::Verified | ModelIntegrity::PresentUnverified => {}
            ModelIntegrity::NotInstalled => return Err(ActivateError::NotInstalled),
            ModelIntegrity::Corrupt(reason) => return Err(ActivateError::Corrupt(reason.clone())),
        }

        let core = TranscribeService::builder()
            .model_path(self.model_path_for(&model))
            .build();

        let mut guard = self.active.write().await;
        guard.core = core;
        guard.model = model;
        drop(guard);

        Ok(integrity)
    }

    /// Startup reconciliation (design doc §8): verify every catalog model's
    /// on-disk reality (existence + size + CRC32) and quarantine anything
    /// corrupt to `*.corrupt`, exactly like the desktop's plugin `setup()`
    /// hook (`plugins/local-stt/src/lib.rs`) — reused verbatim via
    /// `ModelDownloadManager::reconcile`, not reimplemented. Call once from
    /// `main` before serving.
    pub async fn reconcile_on_startup(&self) {
        let models: Vec<LocalModel> = CATALOG.iter().cloned().map(LocalModel::Whisper).collect();
        let results = self.downloader.reconcile(&models).await;
        tracing::info!(
            checked = results.len(),
            "stt_server_model_reconcile_done"
        );
    }
}
