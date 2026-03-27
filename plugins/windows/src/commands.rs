use crate::{AppWindow, WindowsPluginExt, events};

use tauri::Manager;

#[tauri::command]
#[specta::specta]
pub async fn control_set_always_on_top(
    app: tauri::AppHandle<tauri::Wry>,
    always_on_top: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("control")
        .ok_or("control window not found")?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn control_set_opacity(
    app: tauri::AppHandle<tauri::Wry>,
    opacity: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("control")
        .ok_or("control window not found")?;

    #[cfg(target_os = "macos")]
    {
        if let Ok(ns_win) = window.ns_window() {
            unsafe {
                let ns_window = &*(ns_win as *mut objc2_app_kit::NSWindow);
                ns_window.setAlphaValue(opacity);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = opacity;
        let _ = window;
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_show(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows()
        .show_async(window)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_destroy(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<(), String> {
    app.windows().destroy(window).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    path: String,
) -> Result<(), String> {
    app.windows()
        .navigate(window, path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_emit_navigate(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
    event: events::Navigate,
) -> Result<(), String> {
    app.windows()
        .emit_navigate(window, event)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn window_is_exists(
    app: tauri::AppHandle<tauri::Wry>,
    window: AppWindow,
) -> Result<bool, String> {
    let exists = app.windows().is_exists(window).map_err(|e| e.to_string())?;
    Ok(exists)
}
