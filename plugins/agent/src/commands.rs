use crate::AgentPluginExt;

#[tauri::command]
#[specta::specta]
pub fn ping<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    payload: hypr_agent_core::PingRequest,
) -> Result<hypr_agent_core::PingResponse, String> {
    Ok(app.agent().ping(payload))
}
