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

/// Register a keyed global hotkey, backed by `tauri-plugin-global-shortcut` on
/// every platform (macOS included, since #31). Several hotkeys can be live at
/// once, each under its own `id` (e.g. the dictation toggle and paste-last
/// hotkeys); re-registering an `id` replaces its previous binding without
/// disturbing the others. Emits `GlobalHotkeyTriggered { id, shortcut }` on
/// key-down.
#[tauri::command]
#[specta::specta]
pub(crate) async fn register_global_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
    shortcut: String,
) -> Result<(), String> {
    app.shortcut()
        .register_global(id, shortcut)
        .map_err(|e| e.to_string())
}

/// Unregister the keyed global hotkey previously registered under `id`. A no-op
/// if that id has no live binding.
#[tauri::command]
#[specta::specta]
pub(crate) async fn unregister_global_hotkey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<(), String> {
    app.shortcut()
        .unregister_global(id)
        .map_err(|e| e.to_string())
}

/// Parse-validate a global-hotkey accelerator string (e.g. "ctrl+alt+space")
/// WITHOUT registering it, so the settings recorder can show inline feedback
/// before committing the `dictation_shortcut` setting.
#[tauri::command]
#[specta::specta]
pub(crate) async fn parse_global_hotkey(shortcut: String) -> Result<(), String> {
    crate::handler::parse_global(&shortcut).map_err(|e| e.to_string())
}
