use crate::{
    events::{HotKey, Options, Permissions},
    ext::ShortcutPluginExt,
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn register_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    hotkey: HotKey,
    options: Options,
) -> Result<(), String> {
    app.shortcut()
        .register(hotkey, options)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unregister_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.shortcut().unregister().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn check_permissions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Permissions, String> {
    Ok(app.shortcut().check_permissions())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn request_accessibility_permission<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.shortcut()
        .request_accessibility_permission()
        .map_err(|e| e.to_string())
}
