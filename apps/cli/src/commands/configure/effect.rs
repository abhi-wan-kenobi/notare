use hypr_db_app::CalendarRow;

pub enum Effect {
    Exit,
    LoadSettings,
    SaveSttProvider(String),
    SaveLlmProvider(String),
    LoadCalendars,
    SaveCalendars(Vec<CalendarRow>),
    CheckCalendarPermission,
    RequestCalendarPermission,
    ResetCalendarPermission,
}
