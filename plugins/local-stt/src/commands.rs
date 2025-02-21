use crate::LocalSttPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn load_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    on_progress: tauri::ipc::Channel<u8>,
) -> Result<(), String> {
    app.load_model(on_progress).await
}

#[tauri::command]
#[specta::specta]
pub async fn unload_model<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.unload_model()
}

#[tauri::command]
#[specta::specta]
pub async fn start_server<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.start_server().await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_server<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.stop_server().await
}
