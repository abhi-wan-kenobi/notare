use hypr_calendar_interface::{
    CalendarEvent, CalendarListItem, CalendarProviderType, CreateEventInput, EventFilter,
};
use tauri::Manager;
use tauri_plugin_auth::AuthPluginExt;
// Only used by the macOS calendar-permission check below; gate the import to the
// same cfg so non-macOS builds don't see it as unused (it looks dead on Linux —
// the .permissions() call is in a #[cfg(target_os = "macos")] block).
#[cfg(target_os = "macos")]
use tauri_plugin_permissions::PermissionsPluginExt;

use crate::error::Error;

#[tauri::command]
#[specta::specta]
pub fn available_providers() -> Vec<CalendarProviderType> {
    hypr_calendar::available_providers()
}

#[tauri::command]
#[specta::specta]
pub async fn is_provider_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
) -> Result<bool, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = access_token(&app);
    let apple = is_apple_authorized(&app).await?;
    let google = crate::google::is_connected(&app).await;
    let ics = crate::ics::has_files(&app);
    hypr_calendar::is_provider_enabled(
        &config.api_base_url,
        token.as_deref(),
        apple,
        google,
        ics,
        provider,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_connection_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<hypr_calendar::ProviderConnectionIds>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = access_token(&app);
    let apple = is_apple_authorized(&app).await?;
    let google = crate::google::is_connected(&app).await;
    let ics = crate::ics::has_files(&app);
    hypr_calendar::list_connection_ids(&config.api_base_url, token.as_deref(), apple, google, ics)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_calendars<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    connection_id: String,
) -> Result<Vec<CalendarListItem>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = match provider {
        CalendarProviderType::Apple => access_token(&app).unwrap_or_default(),
        CalendarProviderType::Google => crate::google::access_token(&app).await?,
        CalendarProviderType::Ics => String::new(),
        _ => require_access_token(&app)?,
    };
    let ics_dir = crate::ics::ics_dir(&app)?;
    hypr_calendar::list_calendars(
        &config.api_base_url,
        &token,
        provider,
        &connection_id,
        Some(&ics_dir),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn list_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    connection_id: String,
    filter: EventFilter,
) -> Result<Vec<CalendarEvent>, Error> {
    let config = app.state::<crate::PluginConfig>();
    let token = match provider {
        CalendarProviderType::Apple => access_token(&app).unwrap_or_default(),
        CalendarProviderType::Google => crate::google::access_token(&app).await?,
        CalendarProviderType::Ics => String::new(),
        _ => require_access_token(&app)?,
    };
    let ics_dir = crate::ics::ics_dir(&app)?;
    hypr_calendar::list_events(
        &config.api_base_url,
        &token,
        provider,
        &connection_id,
        filter,
        Some(&ics_dir),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn open_calendar<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
) -> Result<(), Error> {
    hypr_calendar::open_calendar(provider).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn create_event<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: CalendarProviderType,
    input: CreateEventInput,
) -> Result<String, Error> {
    hypr_calendar::create_event(provider, input).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn parse_meeting_link(text: String) -> Option<String> {
    hypr_calendar::parse_meeting_link(&text)
}

// --- Direct (BYO OAuth client) Google integration ---

#[tauri::command]
#[specta::specta]
pub async fn google_account_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<crate::google::GoogleAccountStatus, Error> {
    crate::google::status(&app).await
}

#[tauri::command]
#[specta::specta]
pub async fn google_import_client_json<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    json: String,
) -> Result<crate::google::GoogleClientImportResult, Error> {
    crate::google::import_client_json(&app, &json).await
}

#[tauri::command]
#[specta::specta]
pub async fn google_import_client_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<crate::google::GoogleClientImportResult, Error> {
    crate::google::import_client_file(&app, &path).await
}

/// Opens the browser consent screen and waits (up to 5 minutes) for the user
/// to finish; returns the updated status.
#[tauri::command]
#[specta::specta]
pub async fn google_connect<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<crate::google::GoogleAccountStatus, Error> {
    crate::google::connect(&app).await
}

/// Revokes + forgets the session but keeps the imported client json.
#[tauri::command]
#[specta::specta]
pub async fn google_disconnect<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<crate::google::GoogleAccountStatus, Error> {
    crate::google::disconnect(&app).await
}

/// Removes the session AND the imported client json.
#[tauri::command]
#[specta::specta]
pub async fn google_reset<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<crate::google::GoogleAccountStatus, Error> {
    crate::google::reset(&app).await
}

// --- Imported .ics calendar files ---

#[tauri::command]
#[specta::specta]
pub fn ics_list_files<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<crate::ics::IcsImportedFile>, Error> {
    crate::ics::list(&app)
}

/// Copy one or more picked `.ics` files into the app data dir; each becomes
/// its own calendar.
#[tauri::command]
#[specta::specta]
pub fn ics_import_files<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    paths: Vec<String>,
) -> Result<Vec<crate::ics::IcsImportedFile>, Error> {
    crate::ics::import(&app, &paths)
}

/// Replace the stored copy of an imported calendar with a fresh file (keeps
/// the calendar id, so enabled-state and synced events stay attached).
#[tauri::command]
#[specta::specta]
pub fn ics_replace_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
    path: String,
) -> Result<crate::ics::IcsImportedFile, Error> {
    crate::ics::replace(&app, &id, &path)
}

/// Remove an imported calendar file (its events disappear on the next sync).
#[tauri::command]
#[specta::specta]
pub fn ics_remove_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<(), Error> {
    crate::ics::remove(&app, &id)
}

fn access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    app.access_token().ok().flatten().filter(|t| !t.is_empty())
}

fn require_access_token<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, Error> {
    let token = app.access_token().map_err(|e| Error::Auth(e.to_string()))?;
    match token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(hypr_calendar::Error::NotAuthenticated.into()),
    }
}

async fn is_apple_authorized<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    {
        let status = app
            .permissions()
            .check(tauri_plugin_permissions::Permission::Calendar)
            .await
            .map_err(|e| hypr_calendar::Error::Api(e.to_string()))?;
        Ok(matches!(
            status,
            tauri_plugin_permissions::PermissionStatus::Authorized
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(false)
    }
}
