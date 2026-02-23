use std::path::PathBuf;

use hypr_listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent,
};
use tokio::sync::mpsc;

pub enum ListenerEvent {
    Lifecycle(SessionLifecycleEvent),
    Progress(SessionProgressEvent),
    Error(SessionErrorEvent),
    Data(SessionDataEvent),
}

pub struct TuiRuntime {
    sessions_dir: PathBuf,
    tx: mpsc::UnboundedSender<ListenerEvent>,
}

impl TuiRuntime {
    pub fn new(sessions_dir: PathBuf, tx: mpsc::UnboundedSender<ListenerEvent>) -> Self {
        Self { sessions_dir, tx }
    }
}

impl ListenerRuntime for TuiRuntime {
    fn sessions_dir(&self) -> Result<PathBuf, String> {
        Ok(self.sessions_dir.clone())
    }

    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        let _ = self.tx.send(ListenerEvent::Lifecycle(event));
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        let _ = self.tx.send(ListenerEvent::Progress(event));
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        let _ = self.tx.send(ListenerEvent::Error(event));
    }

    fn emit_data(&self, event: SessionDataEvent) {
        let _ = self.tx.send(ListenerEvent::Data(event));
    }
}
