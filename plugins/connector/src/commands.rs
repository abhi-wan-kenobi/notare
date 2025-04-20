use crate::{ConnectionType, ConnectorPluginExt};

#[tauri::command]
#[specta::specta]
pub async fn get_api_base<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    t: ConnectionType,
) -> Result<Option<String>, String> {
    app.get_api_base(t).await.map_err(|e| e.to_string())
}
