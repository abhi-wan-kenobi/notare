use std::path::PathBuf;

use rig::message::Message;

pub(crate) enum Effect {
    Submit {
        prompt: String,
        history: Vec<Message>,
    },
    GenerateTitle {
        prompt: String,
        response: String,
    },
    Persist {
        db_path: PathBuf,
        session_id: String,
        message_id: String,
        role: String,
        content: String,
    },
    UpdateTitle {
        db_path: PathBuf,
        session_id: String,
        title: String,
    },
    Exit,
}
