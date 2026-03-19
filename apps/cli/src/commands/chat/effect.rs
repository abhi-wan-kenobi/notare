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
        session_id: String,
        message_id: String,
        role: String,
        content: String,
    },
    UpdateTitle {
        session_id: String,
        title: String,
    },
    Exit,
}
