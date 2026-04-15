pub use hypr_activity_capture_interface::*;
pub mod observation;
pub use observation::*;
pub mod screenshot;
pub use screenshot::*;
pub mod storage;
pub use storage::*;

#[cfg(target_os = "macos")]
pub type PlatformCapture = hypr_activity_capture_macos::MacosCapture;

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Clone, Default)]
pub struct PlaceholderCapture {
    #[allow(dead_code)]
    policy: CapturePolicy,
}

#[cfg(not(target_os = "macos"))]
impl PlaceholderCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }
}

#[cfg(not(target_os = "macos"))]
impl ActivityCapture for PlaceholderCapture {
    fn capabilities(&self) -> Capabilities {
        // Non-macOS activity capture is still a placeholder until that platform is implemented.
        Capabilities::default()
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        Err(CaptureError::unsupported(
            "activity capture is only implemented on macOS today",
        ))
    }

    fn watch(&self, _options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        Err(CaptureError::unsupported(
            "activity capture is only implemented on macOS today",
        ))
    }
}

#[cfg(not(target_os = "macos"))]
pub type PlatformCapture = PlaceholderCapture;
