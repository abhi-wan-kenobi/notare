use crate::{ConnectionType, ConnectorPluginExt};

#[tauri::command]
#[specta::specta]
pub async fn get_api_base<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    t: ConnectionType,
) -> Option<String> {
    app.get_api_base(t).await
}
