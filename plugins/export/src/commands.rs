use std::path::PathBuf;

use crate::ExportPluginExt;
use crate::action_items::ExportActionItemsFormat;

#[tauri::command]
#[specta::specta]
pub(crate) async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: PathBuf,
    input: crate::ExportInput,
) -> Result<(), String> {
    app.export()
        .export_pdf(&path, input)
        .map_err(|e| e.to_string())
}

/// Export a session's (or, when `session_id` is `None`, every session's) action
/// items to `path` as CSV or JSON. SQLite is authoritative; this is a read-only
/// projection. A `path` argument is included to mirror the file-writing shape of
/// the sibling `export` (PDF) command.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_action_items(
    state: tauri::State<'_, crate::ManagedState>,
    path: PathBuf,
    session_id: Option<String>,
    format: ExportActionItemsFormat,
) -> Result<(), String> {
    let items = crate::action_items::collect_action_items(state.pool(), session_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let payload = crate::action_items::serialize(&items, format).map_err(|e| e.to_string())?;
    std::fs::write(&path, payload).map_err(|e| e.to_string())
}
