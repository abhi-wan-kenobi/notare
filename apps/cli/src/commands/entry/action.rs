use crossterm::event::KeyEvent;

use crate::cli::ConnectionType;
use crate::commands::connect::runtime::RuntimeEvent as ConnectRuntimeEvent;
use crate::commands::model::list::ModelRow;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    SubmitCommand(String),
    StatusMessage(String),
    ConnectRuntime(ConnectRuntimeEvent),
    SessionsLoaded(Vec<hypr_db_app::SessionRow>),
    SessionsLoadError(String),
    ModelsLoaded(Vec<ModelRow>),
    ModelsLoadError(String),
    ConnectSaved {
        connection_types: Vec<ConnectionType>,
        provider_id: String,
    },
}
