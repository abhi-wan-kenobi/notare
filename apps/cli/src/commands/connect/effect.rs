use crate::cli::{ConnectProvider, ConnectionType};

pub(crate) struct SaveData {
    pub connection_type: ConnectionType,
    pub provider: ConnectProvider,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub(crate) enum Effect {
    Save(SaveData),
    Exit,
}
