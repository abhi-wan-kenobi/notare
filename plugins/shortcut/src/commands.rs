use crate::{
    events::{HotKey, Options},
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

/// Register a toggle-style global hotkey (Windows/Linux). Emits
/// `GlobalHotkeyTriggered` on key-down. Not available on macOS, which keeps
/// its native push-to-talk path.
#[tauri::command]
#[specta::specta]
pub(crate) async fn register_global_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    shortcut: String,
) -> Result<(), String> {
    app.shortcut()
        .register_global(shortcut)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unregister_global_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    app.shortcut()
        .unregister_global()
        .map_err(|e| e.to_string())
}

/// Parse-validate a global-hotkey accelerator string (e.g. "ctrl+alt+space")
/// WITHOUT registering it, so the settings recorder can show inline feedback
/// before committing the `dictation_shortcut` setting. Always Ok on macOS,
/// which keeps its native push-to-talk path and never parses these strings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn parse_global_hotkey(shortcut: String) -> Result<(), String> {
    crate::handler::parse_global(&shortcut).map_err(|e| e.to_string())
}
