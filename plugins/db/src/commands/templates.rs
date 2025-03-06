#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn list_templates(
    state: tauri::State<'_, crate::ManagedState>,
) -> Result<Vec<hypr_db_user::Template>, String> {
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

    db.list_templates(user_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn upsert_template(
    state: tauri::State<'_, crate::ManagedState>,
    template: hypr_db_user::Template,
) -> Result<hypr_db_user::Template, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.upsert_template(template)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn delete_template(
    state: tauri::State<'_, crate::ManagedState>,
    id: String,
) -> Result<(), String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.delete_template(id).await.map_err(|e| e.to_string())
}
