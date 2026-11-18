use tauri::State;

use crate::error::Result;
use crate::ext::McpServerManager;
use crate::types::{McpServerSettings, McpServerSettingsInput, McpServerStatus};

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, McpServerManager>) -> Result<McpServerSettings, String> {
    state.get_settings().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_settings(
    state: State<'_, McpServerManager>,
    settings: McpServerSettingsInput,
) -> Result<McpServerSettings, String> {
    state
        .set_settings(settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn regenerate_token(state: State<'_, McpServerManager>) -> Result<String, String> {
    state
        .regenerate_token_and_restart()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_status(state: State<'_, McpServerManager>) -> Result<McpServerStatus, String> {
    Ok(state.get_status().await)
}

#[tauri::command]
#[specta::specta]
/// Absolute path of the bundled `notare-mcp` stdio bridge (a Tauri
/// sidecar), or `None` in builds that do not carry it (dev runs). The
/// Settings page puts it into the Claude Desktop snippet so the copied
/// config works without the user locating the binary.
pub async fn get_bridge_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(resolve_bridge_path(&app))
}

fn resolve_bridge_path(app: &tauri::AppHandle) -> Option<String> {
    #[cfg(target_os = "windows")]
    let bin_name = "notare-mcp.exe";
    #[cfg(not(target_os = "windows"))]
    let bin_name = "notare-mcp";

    // 1. Check next to the running executable:
    // On macOS: <App>.app/Contents/MacOS/notare-mcp
    // On Windows: <InstallDir>/notare-mcp.exe
    // On Linux AppImage / deb: /usr/bin/notare-mcp (or squashfs /usr/bin/notare-mcp)
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_path = std::fs::canonicalize(&exe_path).unwrap_or(exe_path);
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    // 2. Check resource directory:
    use tauri::Manager;
    if let Ok(resource_dir) = app.path().resource_dir() {
        let res_candidate = resource_dir.join(bin_name);
        if res_candidate.is_file() {
            return Some(res_candidate.to_string_lossy().to_string());
        }
        let res_binaries_candidate = resource_dir.join("binaries").join(bin_name);
        if res_binaries_candidate.is_file() {
            return Some(res_binaries_candidate.to_string_lossy().to_string());
        }
    }

    None
}
