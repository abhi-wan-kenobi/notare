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
        connection_types: Vec<ConnectionType>,
        provider_id: String,
    },
}
