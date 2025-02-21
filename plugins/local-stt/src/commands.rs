use crate::LocalSttPluginExt;

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn load_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    on_progress: tauri::ipc::Channel<u8>,
) -> Result<(), String> {
    app.load_model(on_progress).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn unload_model<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.unload_model().map_err(|e| e.to_string())
}
