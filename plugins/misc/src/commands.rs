use crate::ext::UtilsPluginExt;
use crate::model::TemplateName;

#[tauri::command]
#[specta::specta]
pub async fn opinionated_md_to_html<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: String,
) -> Result<String, String> {
    app.opinionated_md_to_html(&text)
}

#[tauri::command]
#[specta::specta]
pub async fn list_template_names<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<TemplateName>, String> {
    Ok(app.list_template_names())
}
