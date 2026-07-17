//! Imported `.ics` calendar-file management for the calendar plugin.
//!
//! Files picked by the user are copied into `<app-data>/calendars/ics/` (via
//! `hypr_ics_calendar::IcsStore`) so the source can disappear; each imported
//! file is one calendar under the synthetic `ics` connection.

use std::path::PathBuf;

use tauri::Manager;

use crate::error::Error;

/// One imported `.ics` file, surfaced to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct IcsImportedFile {
    /// Stable id — also the calendar tracking id in the DB.
    pub id: String,
    /// Original file name at import time.
    pub file_name: String,
    /// `X-WR-CALNAME`, when the file has one.
    pub calendar_name: Option<String>,
    /// Display name (calendar name, else file name without extension).
    pub title: String,
    pub event_count: u32,
    pub imported_at: String,
    pub updated_at: String,
}

impl From<hypr_ics_calendar::IcsFileInfo> for IcsImportedFile {
    fn from(info: hypr_ics_calendar::IcsFileInfo) -> Self {
        Self {
            id: info.id,
            file_name: info.file_name,
            calendar_name: info.calendar_name,
            title: info.title,
            event_count: info.event_count,
            imported_at: info.imported_at,
            updated_at: info.updated_at,
        }
    }
}

/// Directory the stored copies live in: `<app-data>/calendars/ics/`.
pub fn ics_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, Error> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| Error::Auth(format!("could not resolve the app data dir: {e}")))?;
    Ok(base.join("calendars").join("ics"))
}

pub fn store<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<hypr_ics_calendar::IcsStore, Error> {
    Ok(hypr_ics_calendar::IcsStore::new(ics_dir(app)?))
}

/// Cheap connected-check used by the provider plumbing.
pub fn has_files<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    store(app).map(|s| s.has_files()).unwrap_or(false)
}

pub fn list<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Vec<IcsImportedFile>, Error> {
    let files = store(app)?.list().map_err(to_error)?;
    Ok(files.into_iter().map(Into::into).collect())
}

pub fn import<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    paths: &[String],
) -> Result<Vec<IcsImportedFile>, Error> {
    let store = store(app)?;
    let mut imported = Vec::new();
    for path in paths {
        imported.push(
            store
                .import(std::path::Path::new(path))
                .map_err(to_error)?
                .into(),
        );
    }
    Ok(imported)
}

pub fn replace<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
    path: &str,
) -> Result<IcsImportedFile, Error> {
    store(app)?
        .replace(id, std::path::Path::new(path))
        .map(Into::into)
        .map_err(to_error)
}

pub fn remove<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) -> Result<(), Error> {
    store(app)?.remove(id).map_err(to_error)
}

fn to_error(e: hypr_ics_calendar::Error) -> Error {
    Error::Calendar(hypr_calendar::Error::Ics(e))
}
