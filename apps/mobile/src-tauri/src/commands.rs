use crate::AppExt;

#[tauri::command]
#[specta::specta]
pub async fn setup_db<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.setup_db().await.map_err(|e| e.to_string())
}
