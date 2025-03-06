#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn list_calendars(
    state: tauri::State<'_, crate::ManagedState>,
) -> Result<Vec<hypr_db_user::Calendar>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.list_calendars().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn list_participants(
    state: tauri::State<'_, crate::ManagedState>,
    event_id: String,
) -> Result<Vec<hypr_db_user::Human>, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.list_participants(event_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip(state))]
pub async fn upsert_calendar(
    state: tauri::State<'_, crate::ManagedState>,
    calendar: hypr_db_user::Calendar,
) -> Result<hypr_db_user::Calendar, String> {
    let guard = state.lock().await;

    let db = guard
        .db
        .as_ref()
        .ok_or(crate::Error::NoneDatabase)
        .map_err(|e| e.to_string())?;

    db.upsert_calendar(calendar)
        .await
        .map_err(|e| e.to_string())
}
