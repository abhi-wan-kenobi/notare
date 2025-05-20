use crate::TaskPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn ping<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    app.ping().map_err(|e| e.to_string())
}
