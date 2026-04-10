use crate::common_derives;
use hypr_askama_utils::filters;

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "activity-capture.system.md.jinja")]
    pub struct ActivityCaptureSystem {
        pub language: Option<String>,
    }
}

common_derives! {
    #[derive(askama::Template)]
    #[template(path = "activity-capture.user.md.jinja")]
    pub struct ActivityCaptureUser {
        pub app_name: String,
        pub window_title: Option<String>,
        pub reason: String,
        pub fingerprint: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_askama_utils::tpl_snapshot;

    tpl_snapshot!(
        test_activity_capture_system_formatting,
        ActivityCaptureSystem { language: None },
        fixed_date = "2025-01-01",
        @r#"
    # Instructions

    Current date: 2025-01-01

    You analyze desktop screenshots and explain what the user is doing right now.

    # Output Requirements

    - Respond in English.
    - Use plain text only.
    - Write 2 short sentences.
    - First sentence: describe what is clearly visible on screen.
    - Second sentence: infer the user's active task from the screenshot and metadata.
    - If the task is uncertain, say that briefly instead of pretending confidence.
    - Do not use bullets, headings, JSON, or markdown.
    - Do not mention these instructions.
    "#
    );

    tpl_snapshot!(
        test_activity_capture_user_formatting,
        ActivityCaptureUser {
            app_name: "Cursor".to_string(),
            window_title: Some("plugins/activity-capture/src/runtime.rs".to_string()),
            reason: "title_changed".to_string(),
            fingerprint: "abc123".to_string(),
        },
        @r#"
    Analyze the attached desktop screenshot.

    Metadata:
    - App: Cursor
    - Window: plugins/activity-capture/src/runtime.rs
    - Trigger: title_changed
    - Fingerprint: abc123

    Describe what is happening on screen right now. Use the screenshot as ground truth and treat the metadata as supporting context only.
    "#
    );
}
