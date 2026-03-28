#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2_foundation::{NSAppleScript, NSString};

#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserUrlResolver;

impl BrowserUrlResolver {
    pub fn supports_bundle_id(self, bundle_id: &str) -> bool {
        matches!(
            bundle_id,
            "com.apple.Safari"
                | "com.google.Chrome"
                | "company.thebrowser.Browser"
                | "com.brave.Browser"
                | "com.microsoft.edgemac"
        )
    }

    pub fn current_url(self, bundle_id: &str, _window_title: &str) -> Option<String> {
        let script = match bundle_id {
            "com.apple.Safari" => Some(
                r#"
                tell application "Safari"
                    if (count of windows) is 0 then return ""
                    return URL of current tab of front window
                end tell
                "#,
            )
            .map(str::to_string),
            "com.google.Chrome"
            | "company.thebrowser.Browser"
            | "com.brave.Browser"
            | "com.microsoft.edgemac" => Some(format!(
                r#"
                tell application id "{bundle_id}"
                    if (count of windows) is 0 then return ""
                    return URL of active tab of front window
                end tell
                "#
            )),
            _ => None,
        }?;

        self.run(&script)
    }

    pub fn front_window_is_private(self, bundle_id: &str) -> Option<bool> {
        let script = match bundle_id {
            "com.google.Chrome"
            | "company.thebrowser.Browser"
            | "com.brave.Browser"
            | "com.microsoft.edgemac" => Some(format!(
                r#"
                tell application id "{bundle_id}"
                    if (count of windows) is 0 then return ""
                    return mode of front window
                end tell
                "#
            )),
            _ => None,
        }?;

        self.run(&script)
            .and_then(|value| parse_private_window_mode(&value))
    }

    fn run(self, source: &str) -> Option<String> {
        let source = NSString::from_str(source);
        let script = NSAppleScript::initWithSource(NSAppleScript::alloc(), &source)?;
        let mut error = None;
        let result = unsafe { script.executeAndReturnError(Some(&mut error)) };
        if error.is_some() {
            return None;
        }

        result.stringValue().and_then(|value| {
            let value = value.to_string();
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
    }
}

fn parse_private_window_mode(mode: &str) -> Option<bool> {
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "incognito" | "private" => Some(true),
        "normal" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_private_window_mode;

    #[test]
    fn parse_private_window_mode_detects_private_states() {
        assert_eq!(parse_private_window_mode("incognito"), Some(true));
        assert_eq!(parse_private_window_mode("private"), Some(true));
        assert_eq!(parse_private_window_mode("normal"), Some(false));
        assert_eq!(parse_private_window_mode(""), None);
    }
}
