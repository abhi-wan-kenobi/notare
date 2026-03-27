use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures_util::StreamExt;
use hypr_activity_capture_interface::ActivityCapture;
use tauri_specta::Event;

use crate::events::{
    ActivityCaptureErrorKind, ActivityCapturePluginEvent, ActivityCaptureTransition,
};

pub struct ActivityCaptureRuntime<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    running: AtomicBool,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl<R: tauri::Runtime> ActivityCaptureRuntime<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self {
            app,
            running: AtomicBool::new(false),
            task: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn start(self: &Arc<Self>) -> Result<(), crate::Error> {
        if self.is_running() {
            return Ok(());
        }

        let capture = hypr_activity_capture_macos::MacosCapture::new();
        let mut stream = capture.watch(Default::default())?;

        self.running.store(true, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }

        let runtime = Arc::clone(self);
        let handle = tauri::async_runtime::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(transition) => {
                        let event = ActivityCapturePluginEvent::Transition {
                            transition: ActivityCaptureTransition::from(transition),
                        };
                        if let Err(error) = event.emit(&runtime.app) {
                            tracing::error!(?error, "failed_to_emit_activity_capture_transition");
                        }
                    }
                    Err(error) => {
                        let event = ActivityCapturePluginEvent::Error {
                            kind: ActivityCaptureErrorKind::from(error.kind),
                            message: error.message,
                        };
                        if let Err(emit_error) = event.emit(&runtime.app) {
                            tracing::error!(?emit_error, "failed_to_emit_activity_capture_error");
                        }
                        break;
                    }
                }
            }

            runtime.running.store(false, Ordering::SeqCst);
        });

        *self.task.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }
    }
}
