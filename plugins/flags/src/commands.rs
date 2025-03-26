use crate::FlagsPluginExt;

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    flag: crate::StoreKey,
) -> Result<bool, String> {
    app.is_enabled(flag).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn enable<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    flag: crate::StoreKey,
) -> Result<(), String> {
    app.enable(flag).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn disable<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    flag: crate::StoreKey,
) -> Result<(), String> {
    app.disable(flag).map_err(|e| e.to_string())
}
