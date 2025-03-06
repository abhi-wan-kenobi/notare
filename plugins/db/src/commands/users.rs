#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn get_self_human(
    state: tauri::State<'_, crate::ManagedState>,
) -> Result<Option<hypr_db_user::Human>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    let user_id = guard
        .user_id
        .as_ref()
        .ok_or(crate::Error::NoneUser)
        .map_err(|e| e.to_string())?;

    let human = db.get_human(user_id).await.map_err(|e| e.to_string())?;
    Ok(human)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn upsert_human(
    state: tauri::State<'_, crate::ManagedState>,
    human: hypr_db_user::Human,
) -> Result<hypr_db_user::Human, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.upsert_human(human).await.map_err(|e| e.to_string())
}
