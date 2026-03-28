use std::fs;
use std::process::Command;

use crate::error::{CliError, CliResult};

use super::plist;

pub(crate) fn run() -> CliResult<()> {
    let plist_path = plist::plist_path();

    if !plist_path.exists() {
        eprintln!("Shortcut daemon is not installed.");
        return Ok(());
    }

    let status = Command::new("launchctl")
        .args(["unload", plist_path.to_str().unwrap_or_default()])
        .status()
        .map_err(|e| CliError::operation_failed("launchctl unload", e.to_string()))?;

    if !status.success() {
        eprintln!("Warning: launchctl unload returned non-zero (daemon may not have been running)");
    }

    fs::remove_file(&plist_path)
        .map_err(|e| CliError::operation_failed("remove plist", e.to_string()))?;

    eprintln!("Shortcut daemon uninstalled.");
    Ok(())
}
