use hypr_openrouter::ChatMessage;

pub(crate) enum Effect {
    Submit { messages: Vec<ChatMessage> },
    Exit,
}
