use crate::OutlookCalendarPluginExt;
use crate::error::Error;
use crate::types::EventFilter;
use hypr_calendar_interface::CalendarEvent;
use hypr_outlook_calendar::Calendar;

#[tauri::command]
#[specta::specta]
pub async fn list_calendars<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<Calendar>, Error> {
    app.outlook_calendar().list_calendars().await
}

#[tauri::command]
#[specta::specta]
pub async fn list_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    filter: EventFilter,
) -> Result<Vec<CalendarEvent>, Error> {
    let calendar_id = filter.calendar_tracking_id.clone();
    let events = app.outlook_calendar().list_events(filter).await?;
    Ok(crate::convert::convert_events(events, &calendar_id))
}
