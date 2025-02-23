#[tauri::command]
#[specta::specta]
pub fn get_user_id(app: tauri::State<'_, crate::App>) -> String {
    app.user_id.clone()
}
