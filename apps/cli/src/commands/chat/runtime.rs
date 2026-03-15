use futures_util::StreamExt;
use hypr_openrouter::{ChatCompletionRequest, ChatMessage, Client};
use tokio::sync::mpsc;

pub(crate) enum RuntimeEvent {
    StreamChunk(String),
    StreamCompleted,
    StreamFailed(String),
}

pub(crate) struct Runtime {
    client: Client,
    model: String,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub(crate) fn new(
        client: Client,
        model: String,
        tx: mpsc::UnboundedSender<RuntimeEvent>,
    ) -> Self {
        Self { client, model, tx }
    }

    pub(crate) fn submit(&self, messages: Vec<ChatMessage>) {
        let client = self.client.clone();
        let model = self.model.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let req = build_request(messages, &model);
            let mut stream = match client.chat_completion_stream(&req).await {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = tx.send(RuntimeEvent::StreamFailed(error.to_string()));
                    return;
                }
            };

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first()
                            && let Some(content) = choice.delta.content.as_deref()
                            && tx
                                .send(RuntimeEvent::StreamChunk(content.to_string()))
                                .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(RuntimeEvent::StreamFailed(error.to_string()));
                        return;
                    }
                }
            }

            let _ = tx.send(RuntimeEvent::StreamCompleted);
        });
    }
}

fn build_request(messages: Vec<ChatMessage>, model: &str) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: Some(model.to_string()),
        messages,
        ..Default::default()
    }
}
