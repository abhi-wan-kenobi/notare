use std::path::Path;

const LABEL: &str = "com.char.shortcut";

pub(crate) fn generate(binary_path: &Path) -> String {
    let bin = binary_path.display();
    let home = dirs::home_dir().unwrap_or_default();
    let log_dir = home.join("Library/Logs/char");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>shortcut-daemon</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{log_dir}/shortcut.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/shortcut.stderr.log</string>
</dict>
</plist>
"#,
        log_dir = log_dir.display()
    )
}

pub(crate) fn plist_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub(super) const LAUNCHD_LABEL: &str = LABEL;
