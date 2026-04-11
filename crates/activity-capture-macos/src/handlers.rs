#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{
    ActivityKind, AppIdentity, BrowserContext, CaptureCandidate, CaptureError, SnapshotSource,
};
use objc2_application_services::AXUIElement;

use crate::{app_profile::AppProfile, ax, browser_url::BrowserUrlResolver};

pub(crate) struct CaptureContext<'a> {
    pub app: &'a AppIdentity,
    pub app_profile: AppProfile,
    pub focused_window: &'a AXUIElement,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturePlan {
    pub candidate: CaptureCandidate,
}

pub(crate) fn resolve_capture_plan(
    context: &CaptureContext<'_>,
) -> Result<CapturePlan, CaptureError> {
    Ok(match context.app_profile {
        profile if BrowserUrlResolver.supports_profile(profile) => browser_capture_plan(context)?,
        _ => CapturePlan {
            candidate: foreground_candidate(context),
        },
    })
}

fn browser_capture_plan(context: &CaptureContext<'_>) -> Result<CapturePlan, CaptureError> {
    let raw_url = BrowserUrlResolver
        .current_url(context.app_profile)
        .or(ax::string_attribute(context.focused_window, "AXURL")?);

    Ok(CapturePlan {
        candidate: CaptureCandidate {
            app: context.app.clone(),
            activity_kind: ActivityKind::Browser,
            source: SnapshotSource::Accessibility,
            browser: Some(BrowserContext {
                raw_url,
                is_private: BrowserUrlResolver.front_window_is_private(context.app_profile)
                    == Some(true),
            }),
        },
    })
}

fn foreground_candidate(context: &CaptureContext<'_>) -> CaptureCandidate {
    CaptureCandidate {
        app: context.app.clone(),
        activity_kind: ActivityKind::ForegroundWindow,
        source: SnapshotSource::Accessibility,
        browser: None,
    }
}
