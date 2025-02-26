use crate::SfxPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn play<R: tauri::Runtime>(app: tauri::AppHandle<R>, sfx: crate::AppSounds) {
    app.play(sfx)
}
