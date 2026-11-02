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
    let token = state.regenerate_token().map_err(|e| e.to_string())?;
    // If currently running, restart with the new token
    let status = state.get_status().await;
    if status.running {
        let _ = state.restart(status.port).await;
    }
    Ok(token)
}

#[tauri::command]
#[specta::specta]
pub async fn get_status(state: State<'_, McpServerManager>) -> Result<McpServerStatus, String> {
    Ok(state.get_status().await)
}
