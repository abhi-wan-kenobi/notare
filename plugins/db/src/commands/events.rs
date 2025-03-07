#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn list_events(
    state: tauri::State<'_, crate::ManagedState>,
    user_id: String,
) -> Result<Vec<hypr_db_user::Event>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.list_events(&user_id).await.map_err(|e| e.to_string())
}
