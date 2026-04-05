use std::time::SystemTime;

use hypr_activity_capture::{
    ContentLevel, Event, Snapshot, SnapshotSource, TextAnchorConfidence, TextAnchorKind, Transition,
};

use crate::{
    formatting::{compact, compact_url, format_timestamp},
    ui::{
        APP_PREVIEW_LIMIT, DIFF_PREVIEW_LIMIT, TEXT_PREVIEW_LIMIT, TITLE_PREVIEW_LIMIT,
        URL_PREVIEW_LIMIT,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowStatus {
    Focus,
    Update,
    Idle,
}

impl RowStatus {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Update => "update",
            Self::Idle => "idle",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DetailField {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EventRow {
    pub(crate) captured_at: SystemTime,
    pub(crate) app_name: String,
    pub(crate) status: RowStatus,
    pub(crate) summary: String,
    pub(crate) details: Vec<DetailField>,
}

impl EventRow {
    pub(crate) fn from_transition(transition: &Transition) -> Option<Self> {
        match (transition.previous.as_ref(), transition.current.as_ref()) {
            (None, Some(current)) => Some(Self::focus(
                current.snapshot.captured_at,
                &current.snapshot,
                None,
            )),
            (Some(previous), Some(current)) if same_app(previous, current) => {
                Some(Self::update(previous, current))
            }
            (Some(previous), Some(current)) => Some(Self::focus(
                current.snapshot.captured_at,
                &current.snapshot,
                Some(previous.snapshot.app_name.as_str()),
            )),
            (Some(previous), None) => Some(Self::idle(
                previous.snapshot.captured_at,
                Some(previous.snapshot.app_name.as_str()),
            )),
            (None, None) => None,
        }
    }

    pub(crate) fn idle(captured_at: SystemTime, previous_app: Option<&str>) -> Self {
        let mut details = vec![
            detail("Event", "idle"),
            detail("Captured", format_timestamp(captured_at)),
        ];

        if let Some(previous_app) = previous_app {
            details.push(detail("From", detail_value(previous_app)));
        }

        Self {
            captured_at,
            app_name: "-".to_string(),
            status: RowStatus::Idle,
            summary: previous_app
                .map(|value| format!("from={}", compact(value, APP_PREVIEW_LIMIT)))
                .unwrap_or_else(|| "-".to_string()),
            details,
        }
    }

    fn focus(captured_at: SystemTime, snapshot: &Snapshot, previous_app: Option<&str>) -> Self {
        let mut details = vec![
            detail("Event", "focus"),
            detail("Captured", format_timestamp(captured_at)),
            detail("App", detail_value(&snapshot.app_name)),
        ];

        if let Some(previous_app) = previous_app {
            details.push(detail("From", detail_value(previous_app)));
        }

        push_snapshot_details(&mut details, snapshot);

        Self {
            captured_at,
            app_name: snapshot.app_name.clone(),
            status: RowStatus::Focus,
            summary: focus_summary(snapshot, previous_app),
            details,
        }
    }

    fn update(previous: &Event, current: &Event) -> Self {
        let mut details = vec![
            detail("Event", "update"),
            detail("Captured", format_timestamp(current.snapshot.captured_at)),
            detail("App", detail_value(&current.snapshot.app_name)),
        ];

        push_change_details(
            &mut details,
            "Title",
            previous.snapshot.window_title.as_deref(),
            current.snapshot.window_title.as_deref(),
        );
        push_change_details(
            &mut details,
            "URL",
            previous.snapshot.url.as_deref(),
            current.snapshot.url.as_deref(),
        );
        push_change_details(
            &mut details,
            "Text",
            primary_text(&previous.snapshot),
            primary_text(&current.snapshot),
        );

        if details.len() == 3 {
            details.push(detail("Change", "metadata changed"));
        }

        push_snapshot_details(&mut details, &current.snapshot);

        Self {
            captured_at: current.snapshot.captured_at,
            app_name: current.snapshot.app_name.clone(),
            status: RowStatus::Update,
            summary: update_summary(&previous.snapshot, &current.snapshot),
            details,
        }
    }
}

fn same_app(previous: &Event, current: &Event) -> bool {
    previous.snapshot.bundle_id == current.snapshot.bundle_id
        || previous.snapshot.app_name == current.snapshot.app_name
}

fn focus_summary(snapshot: &Snapshot, previous_app: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(previous_app) = previous_app {
        parts.push(format!("from={}", compact(previous_app, APP_PREVIEW_LIMIT)));
    }
    if let Some(title) = snapshot.window_title.as_deref() {
        parts.push(format!("title={}", display_value("title", title)));
    }
    if let Some(url) = snapshot.url.as_deref() {
        parts.push(format!("url={}", display_value("url", url)));
    } else if let Some(text) = primary_text(snapshot) {
        parts.push(format!("text={}", display_value("text", text)));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join("  ")
    }
}

fn update_summary(previous: &Snapshot, current: &Snapshot) -> String {
    let mut changes = Vec::new();

    push_change(
        &mut changes,
        "title",
        previous.window_title.as_deref(),
        current.window_title.as_deref(),
    );
    push_change(
        &mut changes,
        "url",
        previous.url.as_deref(),
        current.url.as_deref(),
    );
    push_change(
        &mut changes,
        "text",
        primary_text(previous),
        primary_text(current),
    );

    if changes.is_empty() {
        "metadata changed".to_string()
    } else {
        changes.join("  ")
    }
}

fn primary_text(snapshot: &Snapshot) -> Option<&str> {
    snapshot
        .text_anchor_text
        .as_deref()
        .or(snapshot.text_anchor_selected_text.as_deref())
        .or(snapshot.visible_text.as_deref())
}

fn push_change(
    changes: &mut Vec<String>,
    label: &'static str,
    before: Option<&str>,
    after: Option<&str>,
) {
    let before_display = before.map(|value| diff_value(label, value));
    let after_display = after.map(|value| diff_value(label, value));

    if before_display == after_display {
        return;
    }

    match (before_display, after_display) {
        (Some(before), Some(after)) => changes.push(format!("{label}:{before} -> {after}")),
        (None, Some(after)) => changes.push(format!("{label}:+{after}")),
        (Some(before), None) => changes.push(format!("{label}:-{before}")),
        (None, None) => {}
    }
}

fn display_value(label: &str, value: &str) -> String {
    match label {
        "url" => compact_url(value, URL_PREVIEW_LIMIT),
        "text" => compact(value, TEXT_PREVIEW_LIMIT),
        _ => compact(value, TITLE_PREVIEW_LIMIT),
    }
}

fn diff_value(label: &str, value: &str) -> String {
    match label {
        "url" => compact_url(value, DIFF_PREVIEW_LIMIT * 2),
        _ => compact(value, DIFF_PREVIEW_LIMIT),
    }
}

fn push_snapshot_details(details: &mut Vec<DetailField>, snapshot: &Snapshot) {
    details.push(detail("PID", snapshot.pid.to_string()));
    if let Some(bundle_id) = snapshot.bundle_id.as_deref() {
        details.push(detail("Bundle", detail_value(bundle_id)));
    }
    if let Some(title) = snapshot.window_title.as_deref() {
        details.push(detail("Window", detail_value(title)));
    }
    if let Some(url) = snapshot.url.as_deref() {
        details.push(detail("URL", detail_value(url)));
    }
    if let Some(text) = primary_text(snapshot) {
        details.push(detail("Text", detail_value(text)));
    }
    if let Some(anchor_kind) = snapshot.text_anchor_kind {
        details.push(detail("Anchor", anchor_kind_label(anchor_kind)));
    }
    if let Some(anchor_identity) = snapshot.text_anchor_identity.as_deref() {
        details.push(detail("Anchor ID", detail_value(anchor_identity)));
    }
    if let Some(anchor_text) = snapshot.text_anchor_text.as_deref() {
        details.push(detail("Anchor text", detail_value(anchor_text)));
    }
    if let Some(selected_text) = snapshot.text_anchor_selected_text.as_deref() {
        details.push(detail("Selected", detail_value(selected_text)));
    }
    if let Some(prefix) = snapshot.text_anchor_prefix.as_deref() {
        details.push(detail("Prefix", detail_value(prefix)));
    }
    if let Some(suffix) = snapshot.text_anchor_suffix.as_deref() {
        details.push(detail("Suffix", detail_value(suffix)));
    }
    if let Some(confidence) = snapshot.text_anchor_confidence {
        details.push(detail("Confidence", anchor_confidence_label(confidence)));
    }
    details.push(detail(
        "Content",
        content_level_label(snapshot.content_level),
    ));
    details.push(detail("Source", source_label(snapshot.source)));
}

fn push_change_details(
    details: &mut Vec<DetailField>,
    label: &str,
    before: Option<&str>,
    after: Option<&str>,
) {
    if before == after {
        return;
    }

    if let Some(before) = before {
        details.push(detail(format!("{label} before"), detail_value(before)));
    }
    if let Some(after) = after {
        details.push(detail(format!("{label} after"), detail_value(after)));
    }
    if before.is_none() && after.is_none() {
        details.push(detail(label, "-"));
    }
}

fn detail(label: impl Into<String>, value: impl Into<String>) -> DetailField {
    DetailField {
        label: label.into(),
        value: value.into(),
    }
}

fn detail_value(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "-".to_string()
    } else {
        collapsed
    }
}

fn content_level_label(level: ContentLevel) -> &'static str {
    match level {
        ContentLevel::Metadata => "metadata",
        ContentLevel::Url => "url",
        ContentLevel::Full => "full",
    }
}

fn source_label(source: SnapshotSource) -> &'static str {
    match source {
        SnapshotSource::Accessibility => "accessibility",
        SnapshotSource::Workspace => "workspace",
    }
}

fn anchor_kind_label(kind: TextAnchorKind) -> &'static str {
    match kind {
        TextAnchorKind::FocusedEdit => "focused_edit",
        TextAnchorKind::SelectedText => "selected_text",
        TextAnchorKind::FocusedElement => "focused_element",
        TextAnchorKind::Document => "document",
        TextAnchorKind::None => "none",
    }
}

fn anchor_confidence_label(confidence: TextAnchorConfidence) -> &'static str {
    match confidence {
        TextAnchorConfidence::High => "high",
        TextAnchorConfidence::Medium => "medium",
        TextAnchorConfidence::Low => "low",
    }
}
