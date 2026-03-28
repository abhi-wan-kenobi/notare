#![cfg(target_os = "macos")]

use std::{ptr::NonNull, time::SystemTime};

use hypr_activity_capture_interface::{
    ActivityCapture, Capabilities, CaptureAccess, CaptureError, CapturePolicy, CaptureStream,
    ContentLevel, Snapshot, SnapshotSource, WatchOptions,
};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_application_services::{AXError, AXIsProcessTrusted, AXUIElement};
use objc2_core_foundation::{CFArray, CFBoolean, CFRetained, CFString, CFType};
use objc2_foundation::NSString;

use crate::{
    handlers::{CaptureContext, CaptureTextMode, resolve_capture_plan},
    runtime::spawn_watch_stream,
};

const WINDOW_DEPTH_LIMIT: usize = 7;
const WINDOW_NODE_LIMIT: usize = 120;
const FOCUSED_NODE_LIMIT: usize = 40;
const WINDOW_CHARACTER_LIMIT: usize = 2500;
const FOCUSED_CHARACTER_LIMIT: usize = 800;
const ATTRIBUTE_TEXT_LIMIT: usize = 400;

const ATTR_CHILDREN: [&str; 3] = ["AXVisibleChildren", "AXContents", "AXChildren"];
const TEXT_ATTRIBUTES: [&str; 4] = ["AXValue", "AXDescription", "AXTitle", "AXSelectedText"];
const BLOCKED_ROLES: [&str; 16] = [
    "AXButton",
    "AXCheckBox",
    "AXDisclosureTriangle",
    "AXImage",
    "AXIncrementor",
    "AXMenu",
    "AXMenuBar",
    "AXMenuBarItem",
    "AXMenuButton",
    "AXPopUpButton",
    "AXRadioButton",
    "AXScrollBar",
    "AXTab",
    "AXTabGroup",
    "AXToolbar",
    "AXWindow",
];
const PREFERRED_CONTENT_ROLES: [&str; 16] = [
    "AXBrowser",
    "AXCell",
    "AXDocument",
    "AXGroup",
    "AXHeading",
    "AXLayoutArea",
    "AXList",
    "AXListItem",
    "AXOutline",
    "AXRow",
    "AXScrollArea",
    "AXStaticText",
    "AXTable",
    "AXTextArea",
    "AXTextField",
    "AXWebArea",
];

#[derive(Debug, Clone, Default)]
pub struct MacosCapture {
    policy: CapturePolicy,
}

impl MacosCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(policy: CapturePolicy) -> Self {
        Self { policy }
    }

    pub(crate) fn capture_snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        autoreleasepool(|_| {
            let workspace = NSWorkspace::sharedWorkspace();
            let Some(application) = workspace.frontmostApplication() else {
                return Ok(None);
            };
            if application.isHidden() {
                return Ok(None);
            }

            let pid = application.processIdentifier();
            let app_name = application_name(&application);
            let bundle_id = application
                .bundleIdentifier()
                .map(|value| value.to_string());
            let app_access = self.policy.access_for_bundle(bundle_id.as_deref());
            if !app_access.allows_snapshot() {
                return Ok(None);
            }
            if app_access == CaptureAccess::Metadata {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    app_access,
                    None,
                    None,
                    None,
                    SnapshotSource::Workspace,
                )));
            }

            ensure_trusted()?;

            let ax_application = unsafe { AXUIElement::new_application(pid) };
            let focused_window = copy_element_attribute(&ax_application, "AXFocusedWindow")
                .or_else(|_| copy_element_attribute(&ax_application, "AXMainWindow"))?;

            let Some(focused_window) = focused_window else {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    app_access,
                    None,
                    None,
                    None,
                    SnapshotSource::Workspace,
                )));
            };

            if bool_attribute(&focused_window, "AXMinimized")? == Some(true) {
                return Ok(None);
            }

            let bundle_id = bundle_id.filter(|value| !value.is_empty());
            let default_window_title = bundle_id.clone().unwrap_or_else(|| app_name.clone());
            let window_title = string_attribute(&focused_window, "AXTitle")?
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default_window_title.clone());

            let mut plan = resolve_capture_plan(&CaptureContext {
                policy: &self.policy,
                bundle_id: bundle_id.as_deref(),
                window_title: &window_title,
                focused_window: &focused_window,
            })?;
            if plan.url.is_none() && plan.access == CaptureAccess::Url {
                plan.access = CaptureAccess::Metadata;
            }
            if plan.skip || !plan.access.allows_snapshot() {
                return Ok(None);
            }
            if !plan.access.allows_text() {
                return Ok(Some(snapshot_for_access(
                    pid,
                    app_name,
                    bundle_id,
                    plan.access,
                    Some(window_title),
                    plan.url,
                    None,
                    SnapshotSource::Accessibility,
                )));
            }

            let visible_text = match plan.text_mode {
                CaptureTextMode::Generic => {
                    collect_generic_visible_text(&ax_application, &focused_window)?
                }
                CaptureTextMode::Slack => {
                    let visible_text = crate::slack::collect_visible_text(&focused_window)?;
                    if visible_text.is_empty() {
                        collect_generic_visible_text(&ax_application, &focused_window)?
                    } else {
                        visible_text
                    }
                }
                CaptureTextMode::Spotify => {
                    let visible_text = crate::spotify::collect_visible_text()?;
                    if visible_text.is_empty() {
                        collect_generic_visible_text(&ax_application, &focused_window)?
                    } else {
                        visible_text
                    }
                }
            };

            Ok(Some(snapshot_for_access(
                pid,
                app_name,
                bundle_id,
                plan.access,
                Some(window_title),
                plan.url,
                Some(visible_text),
                SnapshotSource::Accessibility,
            )))
        })
    }
}

impl ActivityCapture for MacosCapture {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_watch: true,
            can_capture_visible_text: true,
            can_capture_browser_url: true,
            requires_accessibility_permission: true,
        }
    }

    fn snapshot(&self) -> Result<Option<Snapshot>, CaptureError> {
        self.capture_snapshot()
    }

    fn watch(&self, options: WatchOptions) -> Result<CaptureStream, CaptureError> {
        spawn_watch_stream(self.clone(), options)
    }
}

fn ensure_trusted() -> Result<(), CaptureError> {
    if unsafe { AXIsProcessTrusted() } {
        Ok(())
    } else {
        Err(CaptureError::permission_denied(
            "Accessibility permission is required for activity capture",
        ))
    }
}

fn snapshot_for_access(
    pid: i32,
    app_name: String,
    bundle_id: Option<String>,
    access: CaptureAccess,
    window_title: Option<String>,
    url: Option<String>,
    visible_text: Option<String>,
    source: SnapshotSource,
) -> Snapshot {
    let content_level = match access {
        CaptureAccess::Metadata => ContentLevel::Metadata,
        CaptureAccess::Url => ContentLevel::Url,
        CaptureAccess::Full => ContentLevel::Full,
        CaptureAccess::None => ContentLevel::Metadata,
    };

    Snapshot {
        captured_at: SystemTime::now(),
        pid,
        app_name: app_name.clone(),
        bundle_id,
        window_title: access
            .allows_text()
            .then_some(window_title.filter(|value| !value.is_empty()))
            .flatten(),
        url: access.allows_url().then_some(url).flatten(),
        visible_text: access
            .allows_text()
            .then_some(visible_text.filter(|value| !value.is_empty()))
            .flatten(),
        content_level,
        source: if access == CaptureAccess::Metadata {
            SnapshotSource::Workspace
        } else {
            source
        },
    }
}

fn collect_visible_text(
    element: &AXUIElement,
    depth: usize,
    remaining_nodes: usize,
    remaining_characters: usize,
) -> Result<String, CaptureError> {
    if depth >= WINDOW_DEPTH_LIMIT || remaining_nodes == 0 || remaining_characters == 0 {
        return Ok(String::new());
    }

    let role = string_attribute(element, "AXRole")?.unwrap_or_default();
    if role.to_ascii_lowercase().contains("secure") {
        return Ok(String::new());
    }

    let mut pieces = Vec::new();
    if !BLOCKED_ROLES.contains(&role.as_str()) {
        for attribute in TEXT_ATTRIBUTES {
            let Some(text) = string_attribute(element, attribute)? else {
                continue;
            };
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.len() >= ATTRIBUTE_TEXT_LIMIT {
                continue;
            }
            pieces.push(trimmed.to_string());
        }
    }

    let children = prioritized_children(element)?;
    let child_budget = (remaining_nodes / children.len().max(1)).max(1);
    let char_budget = (remaining_characters / (children.len() + 1).max(1)).max(80);

    for child in children.into_iter().take(remaining_nodes.min(20)) {
        let child_text = collect_visible_text(&child, depth + 1, child_budget, char_budget)?;
        if !child_text.is_empty() {
            pieces.push(child_text);
        }
    }

    let merged = merge_fragments(pieces);
    if merged.len() <= remaining_characters {
        Ok(merged)
    } else {
        Ok(merged.chars().take(remaining_characters).collect())
    }
}

pub(crate) fn child_elements(
    element: &AXUIElement,
) -> Result<Vec<CFRetained<AXUIElement>>, CaptureError> {
    for attribute in ATTR_CHILDREN {
        let Some(value) = copy_attribute_value(element, attribute)? else {
            continue;
        };
        let Ok(array) = value.downcast::<CFArray>() else {
            continue;
        };
        let array = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(array) };

        let mut elements = Vec::new();
        for index in 0..array.len() {
            let Some(item) = array.get(index) else {
                continue;
            };
            let Ok(element) = item.downcast::<AXUIElement>() else {
                continue;
            };
            elements.push(element);
        }

        if !elements.is_empty() {
            return Ok(elements);
        }
    }

    Ok(Vec::new())
}

fn prioritized_children(
    element: &AXUIElement,
) -> Result<Vec<CFRetained<AXUIElement>>, CaptureError> {
    let mut children = child_elements(element)?;

    children.sort_by_key(|child| std::cmp::Reverse(child_priority(child)));
    Ok(children)
}

fn child_priority(element: &AXUIElement) -> usize {
    let role = string_attribute(element, "AXRole")
        .ok()
        .flatten()
        .unwrap_or_default();
    if PREFERRED_CONTENT_ROLES.contains(&role.as_str()) {
        3
    } else if BLOCKED_ROLES.contains(&role.as_str()) {
        0
    } else {
        1
    }
}

fn application_name(application: &NSRunningApplication) -> String {
    application
        .localizedName()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            application
                .bundleIdentifier()
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| application.processIdentifier().to_string())
}

fn copy_element_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<CFRetained<AXUIElement>>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    match value.downcast::<AXUIElement>() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

pub(crate) fn string_attribute(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<String>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    Ok(value
        .downcast_ref::<CFString>()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty()))
}

fn bool_attribute(element: &AXUIElement, attribute: &str) -> Result<Option<bool>, CaptureError> {
    let Some(value) = copy_attribute_value(element, attribute)? else {
        return Ok(None);
    };
    Ok(value
        .downcast_ref::<CFBoolean>()
        .map(|flag| flag == CFBoolean::new(true)))
}

fn copy_attribute_value(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<CFRetained<CFType>>, CaptureError> {
    let attribute = NSString::from_str(attribute);
    let attribute: &CFString = attribute.as_ref();

    let mut raw_value = std::ptr::null();
    let result = unsafe {
        element.copy_attribute_value(
            attribute,
            NonNull::new(&mut raw_value).expect("attribute value pointer is never null"),
        )
    };
    match result {
        value if value == AXError::Success => {
            let Some(value) = NonNull::new(raw_value.cast_mut()) else {
                return Ok(None);
            };
            Ok(Some(unsafe { CFRetained::from_raw(value) }))
        }
        value
            if value == AXError::NoValue
                || value == AXError::AttributeUnsupported
                || value == AXError::Failure
                || value == AXError::CannotComplete =>
        {
            Ok(None)
        }
        error => Err(CaptureError::platform(format!(
            "AX attribute '{attribute}' failed with {:?}",
            error
        ))),
    }
}

pub(crate) fn merge_fragments<I>(fragments: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();

    for fragment in fragments {
        for line in fragment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                lines.push(trimmed.to_string());
            }
        }
    }

    lines.join("\n")
}

fn collect_generic_visible_text(
    ax_application: &AXUIElement,
    focused_window: &AXUIElement,
) -> Result<String, CaptureError> {
    let window_text =
        collect_visible_text(focused_window, 0, WINDOW_NODE_LIMIT, WINDOW_CHARACTER_LIMIT)?;
    let focused_text = copy_element_attribute(ax_application, "AXFocusedUIElement")?
        .map(|value| collect_visible_text(&value, 0, FOCUSED_NODE_LIMIT, FOCUSED_CHARACTER_LIMIT))
        .transpose()?
        .unwrap_or_default();

    Ok(merge_fragments([window_text, focused_text]))
}

#[cfg(test)]
mod tests {
    use super::merge_fragments;

    #[test]
    fn merge_fragments_deduplicates_trimmed_lines() {
        assert_eq!(
            merge_fragments([" hello\nworld ".to_string(), "world\nanother".to_string(),]),
            "hello\nworld\nanother"
        );
    }
}
