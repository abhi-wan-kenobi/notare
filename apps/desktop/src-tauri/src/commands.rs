use crate::{AppExt, StoreKey};

#[tauri::command]
#[specta::specta]
pub async fn sentry_dsn<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    Ok(app.sentry_dsn())
}

#[tauri::command]
#[specta::specta]
pub async fn setup_db_for_cloud<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    app.setup_db_for_cloud().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn is_onboarding_needed<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    let store = app.desktop_store()?;
    store
        .get(StoreKey::OnboardingNeeded)
        .map_err(|e| e.to_string())
        .map(|v| v.unwrap_or(true))
}

#[tauri::command]
#[specta::specta]
pub fn set_onboarding_needed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: bool,
) -> Result<(), String> {
    let store = app.desktop_store()?;
    store
        .set(StoreKey::OnboardingNeeded, v)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_autostart<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    autostart: bool,
) -> Result<(), String> {
    let autostart_manager = {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch()
    };

    if autostart {
        autostart_manager.enable().map_err(|e| e.to_string())
    } else {
        autostart_manager.disable().map_err(|e| e.to_string())
    }
}

// For macOS, try direct clipboard access first, fallback to main thread dispatch if needed
#[tauri::command]
#[specta::specta]
pub async fn clipboard_write_text<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: String,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    let result = app.clipboard().write_text(&text);

    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            tracing::error!("clipboard write failed, trying main thread dispatch");

            #[cfg(target_os = "macos")]
            {
                use std::sync::mpsc;
                let (tx, rx) = mpsc::channel();

                let app_clone = app.clone();
                let text_clone = text.clone();
                app.run_on_main_thread(move || {
                    let result = app_clone.clipboard().write_text(&text_clone);
                    let _ = tx.send(result);
                })
                .map_err(|e| e.to_string())?;

                rx.recv()
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())
            }

            #[cfg(not(target_os = "macos"))]
            {
                Err(e.to_string())
            }
        }
    }
}
