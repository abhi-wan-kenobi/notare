use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{DetectEvent, InstalledApp};

pub(super) struct DetectorState {
    pub(super) last_state: bool,
    last_change: Instant,
    debounce_duration: Duration,
    pub(super) active_apps: Vec<InstalledApp>,
}

impl DetectorState {
    fn new() -> Self {
        Self {
            last_state: false,
            last_change: Instant::now(),
            debounce_duration: Duration::from_millis(500),
            active_apps: Vec::new(),
        }
    }

    fn should_trigger(&mut self, new_state: bool) -> bool {
        let now = Instant::now();
        if new_state == self.last_state {
            return false;
        }
        if now.duration_since(self.last_change) < self.debounce_duration {
            return false;
        }
        self.last_state = new_state;
        self.last_change = now;
        true
    }
}

pub(super) struct SharedContext {
    pub(super) callback: Arc<Mutex<crate::DetectCallback>>,
    pub(super) current_device: Arc<Mutex<Option<cidre::core_audio::Device>>>,
    pub(super) state: Arc<Mutex<DetectorState>>,
    pub(super) polling_active: Arc<AtomicBool>,
}

impl SharedContext {
    pub(super) fn new(callback: crate::DetectCallback) -> Self {
        Self {
            callback: Arc::new(Mutex::new(callback)),
            current_device: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(DetectorState::new())),
            polling_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn clone_shared(&self) -> Self {
        Self {
            callback: self.callback.clone(),
            current_device: self.current_device.clone(),
            state: self.state.clone(),
            polling_active: self.polling_active.clone(),
        }
    }

    pub(super) fn emit(&self, event: DetectEvent) {
        tracing::info!(?event, "detected");
        if let Ok(guard) = self.callback.lock() {
            (*guard)(event);
        }
    }

    pub(super) fn handle_mic_change(&self, mic_in_use: bool) {
        let Ok(mut state_guard) = self.state.lock() else {
            return;
        };

        if !state_guard.should_trigger(mic_in_use) {
            return;
        }

        if mic_in_use {
            let apps = crate::list_mic_using_apps();
            state_guard.active_apps = apps.clone();
            self.polling_active.store(true, Ordering::SeqCst);
            drop(state_guard);
            self.emit(DetectEvent::MicStarted(apps));
        } else {
            self.polling_active.store(false, Ordering::SeqCst);
            let stopped_apps = std::mem::take(&mut state_guard.active_apps);
            drop(state_guard);
            self.emit(DetectEvent::MicStopped(stopped_apps));
        }
    }
}
