#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn upsert_session(
    state: tauri::State<'_, crate::ManagedState>,
    session: hypr_db_user::Session,
) -> Result<hypr_db_user::Session, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.upsert_session(session).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn list_sessions(
    state: tauri::State<'_, crate::ManagedState>,
    filter: Option<hypr_db_user::ListSessionFilter>,
) -> Result<Vec<hypr_db_user::Session>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.list_sessions(filter).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn get_session(
    state: tauri::State<'_, crate::ManagedState>,
    option: hypr_db_user::GetSessionFilter,
) -> Result<Option<hypr_db_user::Session>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.get_session(option).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn visit_session(
    state: tauri::State<'_, crate::ManagedState>,
    id: String,
) -> Result<(), String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.visit_session(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn delete_session(
    state: tauri::State<'_, crate::ManagedState>,
    id: String,
) -> Result<(), String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.delete_session(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn set_session_event(
    state: tauri::State<'_, crate::ManagedState>,
    session_id: String,
    event_id: String,
) -> Result<(), String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.session_set_event(session_id, event_id)
        .await
        .map_err(|e| e.to_string())
}
