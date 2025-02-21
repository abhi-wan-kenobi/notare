use crate::ListenerPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn get_timeline<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<hypr_bridge::TimelineView, String> {
    app.get_timeline().await
}

#[tauri::command]
#[specta::specta]
pub async fn start_session<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.start_session().await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_session<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.stop_session().await
}
