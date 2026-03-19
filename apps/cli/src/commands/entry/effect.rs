use crate::cli::{ConnectProvider, ConnectionType, ModelCommands};

pub(crate) enum Effect {
    Launch(super::EntryCommand),
    LoadSessions,
    SaveConnect {
        connection_types: Vec<ConnectionType>,
        provider: ConnectProvider,
        base_url: Option<String>,
        api_key: Option<String>,
    },
    OpenAuth,
    OpenBug,
    OpenHello,
    OpenDesktop,
    RunModel(ModelCommands),
    Exit,
}
