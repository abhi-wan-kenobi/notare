#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2_foundation::{NSAppleScript, NSString};

#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserUrlResolver;

impl BrowserUrlResolver {
    pub fn current_url(self, bundle_id: &str, window_title: &str) -> Option<String> {
        if is_private_window(window_title) {
            return None;
        }

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

fn is_private_window(title: &str) -> bool {
    let lowered = title.to_ascii_lowercase();
    lowered.contains("private") || lowered.contains("incognito")
}

#[cfg(test)]
mod tests {
    use super::is_private_window;

    #[test]
    fn private_window_titles_are_filtered() {
        assert!(is_private_window("Private Browsing"));
        assert!(is_private_window("Incognito - Example"));
        assert!(!is_private_window("Normal Window"));
    }
}
