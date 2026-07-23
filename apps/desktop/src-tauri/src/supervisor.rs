use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hypr_supervisor::dynamic::{DynamicSupervisor, DynamicSupervisorMsg, DynamicSupervisorOptions};
use ractor::ActorRef;
use ractor::concurrency::Duration;

pub type SupervisorRef = ActorRef<DynamicSupervisorMsg>;
pub type SupervisorHandle = tokio::task::JoinHandle<()>;

const ROOT_SUPERVISOR_NAME: &str = "root_supervisor";

#[derive(Clone)]
pub struct RootSupervisorContext {
    pub supervisor: SupervisorRef,
    pub is_exiting: Arc<AtomicBool>,
}

impl RootSupervisorContext {
    pub fn mark_exiting(&self) {
        self.is_exiting.store(true, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.supervisor.stop(Some("app_exit".to_string()));
    }
}

pub async fn spawn_root_supervisor() -> Option<(RootSupervisorContext, SupervisorHandle)> {
    let options = DynamicSupervisorOptions {
        max_children: Some(10),
        max_restarts: 50,
        max_window: Duration::from_secs(60),
        reset_after: Some(Duration::from_secs(30)),
    };

    match DynamicSupervisor::spawn(ROOT_SUPERVISOR_NAME.to_string(), options).await {
        Ok((supervisor_ref, handle)) => {
            tracing::info!("root_supervisor_spawned");

            let ctx = RootSupervisorContext {
                supervisor: supervisor_ref,
                is_exiting: Arc::new(AtomicBool::new(false)),
            };

            Some((ctx, handle))
        }
        Err(e) => {
            tracing::error!("failed_to_spawn_root_supervisor: {:?}", e);
            None
        }
    }
}

pub fn monitor_supervisor<R: tauri::Runtime>(
    handle: SupervisorHandle,
    is_exiting: Arc<AtomicBool>,
    app_handle: tauri::AppHandle<R>,
) {
    tokio::spawn(async move {
        match handle.await {
            Ok(()) => {
                if !is_exiting.load(Ordering::SeqCst) {
                    tracing::error!("root_supervisor_meltdown");
                    finalize_capture_before_restart(&app_handle).await;
                    app_handle.restart();
                }
            }
            Err(e) => {
                if !is_exiting.load(Ordering::SeqCst) {
                    tracing::error!("root_supervisor_panicked: {:?}", e);
                    finalize_capture_before_restart(&app_handle).await;
                    app_handle.restart();
                }
            }
        }
    });
}

/// DATA-LOSS FIX (macOS recording survives app restart): gate `restart()` behind
/// finalizing any in-flight recording to disk. This is the backstop for the
/// paths the frontend `flushAndExit` can't cover — chiefly a UI HANG (e.g. the
/// speaker-rename bug) that trips this supervisor meltdown while the webview is
/// unresponsive, so the JS-side finalize never runs. Best-effort: `stop_capture`
/// messages the RootActor, which finalizes the session; if the supervisor
/// meltdown already took the actor down there is nothing to flush.
///
/// KNOWN LIMITATION (see the crash/interrupt-resilience hardening pass): if the
/// capture actor tree is already dead, buffered-but-unwritten audio is still
/// lost. True durability needs incremental on-disk persistence of the capture
/// stream, not just finalize-on-teardown.
async fn finalize_capture_before_restart<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    use tauri_plugin_transcription::ListenerPluginExt;
    tracing::info!("finalizing_active_capture_before_restart");
    app_handle.listener().stop_capture().await;
}
