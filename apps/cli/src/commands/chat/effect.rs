use rig::message::Message;

use super::Role;

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
        session_id: String,
        message_id: String,
        role: Role,
        content: String,
    },
    UpdateTitle {
        session_id: String,
        title: String,
    },
    Exit,
}
