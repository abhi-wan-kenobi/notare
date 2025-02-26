use crate::{ShowHyprWindow, WindowsPluginExt};

#[tauri::command]
#[specta::specta]
pub async fn show_window(
    app: tauri::AppHandle<tauri::Wry>,
    window: ShowHyprWindow,
) -> Result<(), String> {
    app.show_window(window).map_err(|e| e.to_string())?;
    Ok(())
}
