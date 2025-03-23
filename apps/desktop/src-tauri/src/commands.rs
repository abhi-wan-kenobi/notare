use crate::ext::AppExt;

#[tauri::command]
#[specta::specta]
pub async fn setup_for_local<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.setup_for_local().await
}
