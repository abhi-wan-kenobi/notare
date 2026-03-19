use crate::cli::{ConnectProvider, ConnectionType, ModelCommands};
use crate::commands::connect::effect::CalendarSaveData;

pub(crate) enum Effect {
    Launch(super::EntryCommand),
    LoadSessions,
    LoadModels,
    LoadTimelineContacts,
    LoadTimelineEntries(String),
    SaveConnect {
        connection_types: Vec<ConnectionType>,
        provider: ConnectProvider,
        base_url: Option<String>,
        api_key: Option<String>,
    },
    CheckCalendarPermission,
    RequestCalendarPermission,
    ResetCalendarPermission,
    LoadCalendars,
    SaveCalendars(CalendarSaveData),
    OpenAuth,
    OpenBug,
    OpenHello,
    OpenDesktop,
    RunModel(ModelCommands),
    Exit,
}
