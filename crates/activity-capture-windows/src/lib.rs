use hypr_activity_capture_interface::{
    ActivityCapture, Capabilities, CaptureError, CapturePolicy, CaptureStream, NormalizedSnapshot,
    WatchOptions,
};

#[derive(Debug, Clone, Default)]
pub struct WindowsCapture {
    #[allow(dead_code)]
    policy: CapturePolicy,
}

impl WindowsCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }
}

impl ActivityCapture for WindowsCapture {
    fn capabilities(&self) -> Capabilities {
        // Windows activity capture is intentionally a placeholder until we build it out.
        Capabilities::default()
    }

    fn snapshot(&self) -> Result<Option<NormalizedSnapshot>, CaptureError> {
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
