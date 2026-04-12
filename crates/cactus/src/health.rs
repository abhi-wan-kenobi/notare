use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex, Once};

use hypr_cactus_model::{CactusServiceHealth, CactusServiceStatus};

static RUNTIME_INIT: Once = Once::new();
static SHARED_STATE: Mutex<SharedHealthState> = Mutex::new(SharedHealthState::new());

#[derive(Default)]
struct SharedHealthState {
    latest_error: Option<String>,
    errors: Vec<String>,
}

impl SharedHealthState {
    const fn new() -> Self {
        Self {
            latest_error: None,
            errors: Vec::new(),
        }
    }

    fn push_error(&mut self, error: String) {
        self.latest_error = Some(error.clone());
        self.errors.push(error);
    }
}

struct ServiceHealthState {
    status: CactusServiceStatus,
    load_error: Option<String>,
}

#[derive(Clone)]
pub struct ServiceHealthTracker {
    state: Arc<Mutex<ServiceHealthState>>,
}

pub fn init_runtime() {
    RUNTIME_INIT.call_once(|| unsafe {
        cactus_sys::cactus_log_set_level(3);
        cactus_sys::cactus_log_set_callback(Some(error_collector), std::ptr::null_mut());
    });
}

pub fn latest_error() -> Option<String> {
    SHARED_STATE.lock().unwrap().latest_error.clone()
}

pub fn latest_errors_snapshot() -> Vec<String> {
    SHARED_STATE.lock().unwrap().errors.clone()
}

pub fn ffi_last_error() -> Option<String> {
    let ptr = unsafe { cactus_sys::cactus_get_last_error() };
    if ptr.is_null() {
        return None;
    }

    let error = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .trim()
        .to_string();
    (!error.is_empty()).then_some(error)
}

impl ServiceHealthTracker {
    pub fn new() -> Self {
        init_runtime();

        Self {
            state: Arc::new(Mutex::new(ServiceHealthState {
                status: CactusServiceStatus::Loading,
                load_error: None,
            })),
        }
    }

    pub fn mark_idle(&self) {
        let mut state = self.state.lock().unwrap();
        state.status = CactusServiceStatus::Idle;
        state.load_error = None;
    }

    pub fn mark_loading(&self) {
        let mut state = self.state.lock().unwrap();
        state.status = CactusServiceStatus::Loading;
        state.load_error = None;
    }

    pub fn mark_ready(&self) {
        let mut state = self.state.lock().unwrap();
        state.status = CactusServiceStatus::Ready;
        state.load_error = None;
    }

    pub fn mark_load_failed(&self, error: impl Into<String>) {
        let error = error.into();
        let mut state = self.state.lock().unwrap();
        state.status = CactusServiceStatus::Failed;
        state.load_error = Some(error);
    }

    pub fn snapshot(&self) -> CactusServiceHealth {
        let state = self.state.lock().unwrap();
        let error = state
            .load_error
            .clone()
            .or_else(|| latest_error())
            .or_else(ffi_last_error);
        let status = state.status;

        CactusServiceHealth { status, error }
    }
}

unsafe extern "C" fn error_collector(
    _level: c_int,
    component: *const c_char,
    message: *const c_char,
    _user_data: *mut c_void,
) {
    let component = unsafe { CStr::from_ptr(component) }.to_str().unwrap_or("");
    let message = unsafe { CStr::from_ptr(message) }.to_str().unwrap_or("");

    let entry = if component.is_empty() {
        message.to_string()
    } else {
        format!("[{component}] {message}")
    };

    SHARED_STATE.lock().unwrap().push_error(entry);
}

#[cfg(test)]
pub(crate) fn record_error_for_tests(error: impl Into<String>) {
    SHARED_STATE.lock().unwrap().push_error(error.into());
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    let mut state = SHARED_STATE.lock().unwrap();
    state.latest_error = None;
    state.errors.clear();
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn init_runtime_is_idempotent() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();

        init_runtime();
        init_runtime();

        assert!(latest_error().is_none());
    }

    #[test]
    fn latest_error_lookup_is_non_destructive() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();

        record_error_for_tests("[init] failed to load model");

        assert_eq!(
            latest_error(),
            Some("[init] failed to load model".to_string())
        );
        assert_eq!(
            latest_error(),
            Some("[init] failed to load model".to_string())
        );
        assert_eq!(
            latest_errors_snapshot(),
            vec!["[init] failed to load model".to_string()]
        );
    }

    #[test]
    fn service_tracker_snapshot_prefers_load_error_then_shared_error() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();

        let tracker = ServiceHealthTracker::new();
        record_error_for_tests("[global] callback error");

        let loading = tracker.snapshot();
        assert_eq!(loading.status, CactusServiceStatus::Loading);
        assert_eq!(loading.error.as_deref(), Some("[global] callback error"));

        tracker.mark_idle();
        let idle = tracker.snapshot();
        assert_eq!(idle.status, CactusServiceStatus::Idle);
        assert_eq!(idle.error.as_deref(), Some("[global] callback error"));

        tracker.mark_load_failed("failed to load model");
        let failed = tracker.snapshot();
        assert_eq!(failed.status, CactusServiceStatus::Failed);
        assert_eq!(failed.error.as_deref(), Some("failed to load model"));

        tracker.mark_ready();
        let ready = tracker.snapshot();
        assert_eq!(ready.status, CactusServiceStatus::Ready);
        assert_eq!(ready.error.as_deref(), Some("[global] callback error"));
    }

    #[test]
    fn service_trackers_keep_independent_status_with_shared_diagnostics() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_for_tests();

        let llm = ServiceHealthTracker::new();
        let transcribe = ServiceHealthTracker::new();

        llm.mark_load_failed("llm failed");
        transcribe.mark_ready();
        record_error_for_tests("[shared] runtime issue");

        let llm_health = llm.snapshot();
        let transcribe_health = transcribe.snapshot();

        assert_eq!(llm_health.status, CactusServiceStatus::Failed);
        assert_eq!(llm_health.error.as_deref(), Some("llm failed"));
        assert_eq!(transcribe_health.status, CactusServiceStatus::Ready);
        assert_eq!(
            transcribe_health.error.as_deref(),
            Some("[shared] runtime issue")
        );
    }
}
