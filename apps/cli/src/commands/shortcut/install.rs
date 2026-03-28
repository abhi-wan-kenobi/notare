use std::fs;
use std::process::Command;

use crate::error::{CliError, CliResult};

use super::plist;

pub(crate) fn run() -> CliResult<()> {
    if !check_input_monitoring() {
        eprintln!("Input Monitoring permission is required for global hotkey listening.");
        eprintln!();
        eprintln!("Opening System Settings → Privacy & Security → Input Monitoring…");
        eprintln!("Grant access to the char binary, then re-run: char shortcut install");
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .status();
        return Ok(());
    }

    let binary_path = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| CliError::operation_failed("resolve binary path", e.to_string()))?;

    let plist_content = plist::generate(&binary_path);
    let plist_path = plist::plist_path();

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::operation_failed("create LaunchAgents dir", e.to_string()))?;
    }

    let log_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Logs/char");
    fs::create_dir_all(&log_dir)
        .map_err(|e| CliError::operation_failed("create log dir", e.to_string()))?;

    // Unload existing agent if present
    if plist_path.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", plist_path.to_str().unwrap_or_default()])
            .output();
    }

    fs::write(&plist_path, plist_content)
        .map_err(|e| CliError::operation_failed("write plist", e.to_string()))?;

    let status = Command::new("launchctl")
        .args(["load", plist_path.to_str().unwrap_or_default()])
        .status()
        .map_err(|e| CliError::operation_failed("launchctl load", e.to_string()))?;

    if !status.success() {
        return Err(CliError::operation_failed(
            "launchctl load",
            "failed to load LaunchAgent",
        ));
    }

    eprintln!("Shortcut daemon installed and running.");
    eprintln!("  Plist: {}", plist_path.display());
    eprintln!("  Binary: {}", binary_path.display());
    eprintln!();
    eprintln!("Double-tap Right Option to start recording.");
    eprintln!("Run `char shortcut uninstall` to remove.");

    Ok(())
}

fn check_input_monitoring() -> bool {
    use super::hotkey::{ProbeResult, probe_event_tap};
    match probe_event_tap() {
        ProbeResult::Ok => true,
        ProbeResult::Denied => false,
    }
}
