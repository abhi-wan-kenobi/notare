use std::sync::Arc;

use hypr_activity_capture::{ActivityCapture, CapturePolicy, PlatformCapture};

use crate::{
    ManagedState,
    events::{
        ActivityCaptureBudget, ActivityCaptureCapabilities, ActivityCaptureScreenshotAnalysis,
        ActivityCaptureSnapshot, ActivityCaptureStatus,
    },
};

pub struct ActivityCaptureExt<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> ActivityCaptureExt<'a, R, M> {
    pub fn capabilities(&self) -> ActivityCaptureCapabilities {
        PlatformCapture::with_policy(self.runtime().policy())
            .capabilities()
            .into()
    }

    pub fn snapshot(&self) -> Result<Option<ActivityCaptureSnapshot>, crate::Error> {
        PlatformCapture::with_policy(self.runtime().policy())
            .snapshot()
            .map(|value| value.map(Into::into))
            .map_err(Into::into)
    }

    pub fn start(&self) -> Result<(), crate::Error> {
        self.runtime().start()
    }

    pub fn stop(&self) {
        self.runtime().stop();
    }

    pub fn is_running(&self) -> bool {
        self.runtime().is_running()
    }

    pub fn latest_screenshot_analysis(&self) -> Option<ActivityCaptureScreenshotAnalysis> {
        self.runtime().latest_screenshot_analysis()
    }

    pub async fn status(&self) -> ActivityCaptureStatus {
        self.runtime().status().await
    }

    pub fn policy(&self) -> CapturePolicy {
        self.runtime().policy()
    }

    pub fn set_policy(&self, policy: CapturePolicy) -> Result<(), crate::Error> {
        self.runtime().set_policy(policy)
    }

    pub fn reset_policy(&self) -> Result<(), crate::Error> {
        self.runtime().reset_policy()
    }

    pub async fn list_analyses_in_range(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<ActivityCaptureScreenshotAnalysis>, String> {
        self.runtime()
            .list_analyses_in_range(start_ms, end_ms)
            .await
    }

    pub fn configure(
        &self,
        budget: Option<ActivityCaptureBudget>,
        analyze_screenshots: Option<bool>,
    ) -> Result<(), crate::Error> {
        self.runtime().configure(budget, analyze_screenshots)
    }

    fn runtime(&self) -> Arc<crate::runtime::ActivityCaptureRuntime<R>> {
        let state = self.manager.state::<ManagedState<R>>();
        Arc::clone(&*state)
    }
}

pub trait ActivityCapturePluginExt<R: tauri::Runtime> {
    fn activity_capture(&self) -> ActivityCaptureExt<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ActivityCapturePluginExt<R> for T {
    fn activity_capture(&self) -> ActivityCaptureExt<'_, R, Self>
    where
        Self: Sized,
    {
        ActivityCaptureExt {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
