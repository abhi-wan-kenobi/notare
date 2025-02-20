use crate::ext::UtilsPluginExt;

#[tauri::command]
#[specta::specta]
pub async fn opinionated_md_to_html<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: String,
) -> Result<String, String> {
    app.opinionated_md_to_html(&text)
}
