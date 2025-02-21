use futures_util::StreamExt;
use tauri::{ipc::Channel, State};

#[tauri::command]
#[specta::specta]
pub fn get_user_id(app: State<'_, crate::App>) -> String {
    app.user_id.clone()
}
