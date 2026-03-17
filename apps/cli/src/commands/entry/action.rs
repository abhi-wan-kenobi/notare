use crossterm::event::KeyEvent;

use crate::cli::ConnectionType;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    SubmitCommand(String),
    StatusMessage(String),
    SessionsLoaded(Vec<hypr_db_app::SessionRow>),
    SessionsLoadError(String),
    ConnectSaved {
        connection_type: ConnectionType,
        provider_id: String,
    },
}
