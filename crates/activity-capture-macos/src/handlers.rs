#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::{CaptureAccess, CaptureError, CapturePolicy};
use objc2_application_services::AXUIElement;
use url::Url;

use crate::{browser_url::BrowserUrlResolver, slack, spotify};

pub(crate) struct CaptureContext<'a> {
    pub policy: &'a CapturePolicy,
    pub bundle_id: Option<&'a str>,
    pub window_title: &'a str,
    pub focused_window: &'a AXUIElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureTextMode {
    Generic,
    Slack,
    Spotify,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturePlan {
    pub access: CaptureAccess,
    pub url: Option<String>,
    pub skip: bool,
    pub text_mode: CaptureTextMode,
}

type Handler = for<'a> fn(&CaptureContext<'a>) -> Result<Option<CapturePlan>, CaptureError>;

const HANDLERS: [Handler; 3] = [
    spotify_capture_plan,
    slack_capture_plan,
    browser_capture_plan,
];

pub(crate) fn resolve_capture_plan(
    context: &CaptureContext<'_>,
) -> Result<CapturePlan, CaptureError> {
    let app_access = context.policy.access_for_bundle(context.bundle_id);
    if !app_access.allows_snapshot() {
        return Ok(CapturePlan {
            access: app_access,
            url: None,
            skip: true,
            text_mode: CaptureTextMode::Generic,
        });
    }

    for handler in HANDLERS {
        if let Some(mut plan) = handler(context)? {
            plan.access = std::cmp::min(plan.access, app_access);
            plan.skip |= !plan.access.allows_snapshot();
            return Ok(plan);
        }
    }

    Ok(CapturePlan {
        access: normalize_generic_access(app_access),
        url: None,
        skip: false,
        text_mode: CaptureTextMode::Generic,
    })
}

fn slack_capture_plan(context: &CaptureContext<'_>) -> Result<Option<CapturePlan>, CaptureError> {
    let Some(bundle_id) = context.bundle_id else {
        return Ok(None);
    };
    if !slack::supports_bundle_id(bundle_id) {
        return Ok(None);
    }

    Ok(Some(CapturePlan {
        access: normalize_generic_access(context.policy.access_for_bundle(context.bundle_id)),
        url: None,
        skip: false,
        text_mode: CaptureTextMode::Slack,
    }))
}

fn spotify_capture_plan(context: &CaptureContext<'_>) -> Result<Option<CapturePlan>, CaptureError> {
    let Some(bundle_id) = context.bundle_id else {
        return Ok(None);
    };
    if !spotify::supports_bundle_id(bundle_id) {
        return Ok(None);
    }

    Ok(Some(CapturePlan {
        access: normalize_generic_access(context.policy.access_for_bundle(context.bundle_id)),
        url: None,
        skip: false,
        text_mode: CaptureTextMode::Spotify,
    }))
}

fn browser_capture_plan(context: &CaptureContext<'_>) -> Result<Option<CapturePlan>, CaptureError> {
    let Some(bundle_id) = context.bundle_id else {
        return Ok(None);
    };
    if !BrowserUrlResolver.supports_bundle_id(bundle_id) {
        return Ok(None);
    }
    if context.policy.browser.block_private_browsing
        && BrowserUrlResolver.front_window_is_private(bundle_id) == Some(true)
    {
        return Ok(Some(CapturePlan {
            access: CaptureAccess::None,
            url: None,
            skip: true,
            text_mode: CaptureTextMode::Generic,
        }));
    }

    let raw_url = BrowserUrlResolver
        .current_url(bundle_id, context.window_title)
        .or(crate::platform::string_attribute(
            context.focused_window,
            "AXURL",
        )?);
    let (sanitized_url, host) = raw_url
        .as_deref()
        .and_then(|value| sanitize_browser_url(&context.policy.browser, value))
        .map(|(url, host)| (Some(url), host))
        .unwrap_or((None, None));

    let mut access = context
        .policy
        .browser
        .access_for_host(host.as_deref(), context.policy.mode);
    if sanitized_url.is_none() && access > CaptureAccess::Metadata {
        access = CaptureAccess::Metadata;
    }
    if sanitized_url.is_none()
        && access.allows_text()
        && context.policy.browser.require_url_for_text_access
    {
        access = CaptureAccess::Metadata;
    }

    Ok(Some(CapturePlan {
        access,
        url: access.allows_url().then_some(sanitized_url).flatten(),
        skip: !access.allows_snapshot(),
        text_mode: CaptureTextMode::Generic,
    }))
}

fn sanitize_browser_url(
    policy: &hypr_activity_capture_interface::BrowserPolicy,
    raw_url: &str,
) -> Option<(String, Option<String>)> {
    let mut parsed = Url::parse(raw_url).ok()?;
    let host = parsed
        .host_str()
        .map(|value| value.trim_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    if policy.strip_query {
        parsed.set_query(None);
    }
    if policy.strip_fragment {
        parsed.set_fragment(None);
    }

    Some((parsed.to_string(), host))
}

fn normalize_generic_access(access: CaptureAccess) -> CaptureAccess {
    match access {
        CaptureAccess::Url => CaptureAccess::Metadata,
        _ => access,
    }
}
