#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn get_self_organization(
    state: tauri::State<'_, crate::ManagedState>,
) -> Result<hypr_db_user::Organization, String> {
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

    let organization = db
        .get_organization_by_user_id(user_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap();

    Ok(organization)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn upsert_organization(
    state: tauri::State<'_, crate::ManagedState>,
    organization: hypr_db_user::Organization,
) -> Result<hypr_db_user::Organization, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.upsert_organization(organization)
        .await
        .map_err(|e| e.to_string())
}
