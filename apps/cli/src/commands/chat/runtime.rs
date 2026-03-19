use std::sync::{Arc, Mutex};

use rig::message::Message;
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::agent::Backend;
use crate::error::{CliError, CliResult};
use crate::llm::ResolvedLlmConfig;

use super::Role;

pub(crate) enum RuntimeEvent {
    Chunk(String),
    Completed(Option<String>),
    Failed(String),
    TitleGenerated(String),
}

#[derive(Clone)]
pub(crate) struct Runtime {
    backend: Backend,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    max_turns: usize,
    pool: SqlitePool,
    pending_writes: Arc<Mutex<JoinSet<()>>>,
}

impl Runtime {
    pub(crate) fn new(
        config: ResolvedLlmConfig,
        system_message: Option<String>,
        tx: mpsc::UnboundedSender<RuntimeEvent>,
        pool: SqlitePool,
    ) -> CliResult<Self> {
        Ok(Self {
            backend: Backend::new(config, system_message)?,
            tx,
            max_turns: 1,
            pool,
            pending_writes: Arc::new(Mutex::new(JoinSet::new())),
        })
    }

    pub(crate) fn generate_title(&self, prompt: String, response: String) {
        let backend = self.backend.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let title_prompt = format!(
                "Generate a short title (3-5 words) for this conversation. Reply with ONLY the title, no quotes or punctuation.\n\nUser: {prompt}\nAssistant: {response}"
            );
            let result = backend
                .stream_text(title_prompt, Vec::new(), 1, |_| Ok(()))
                .await;
            if let Ok(Some(title)) = result {
                let title = title.trim().to_string();
                if !title.is_empty() {
                    let _ = tx.send(RuntimeEvent::TitleGenerated(title));
                }
            }
        });
    }

    pub(crate) async fn ensure_meeting(&self, meeting_id: &str) {
        let _ = hypr_db_app::insert_meeting(&self.pool, meeting_id, None).await;
    }

    pub(crate) fn persist_message(
        &self,
        meeting_id: String,
        message_id: String,
        role: Role,
        content: String,
    ) {
        let pool = self.pool.clone();
        let role = role.to_string();
        self.pending_writes.lock().unwrap().spawn(async move {
            let _ =
                hypr_db_app::insert_chat_message(&pool, &message_id, &meeting_id, &role, &content)
                    .await;
        });
    }

    pub(crate) fn update_title(&self, meeting_id: String, title: String) {
        let pool = self.pool.clone();
        self.pending_writes.lock().unwrap().spawn(async move {
            let _ = hypr_db_app::update_meeting(&pool, &meeting_id, Some(&title)).await;
        });
    }

    pub(crate) async fn drain_pending_writes(&self) {
        let mut set = {
            let mut guard = self.pending_writes.lock().unwrap();
            std::mem::replace(&mut *guard, JoinSet::new())
        };
        while set.join_next().await.is_some() {}
    }

    pub(crate) fn submit(&self, prompt: String, history: Vec<Message>) {
        let backend = self.backend.clone();
        let tx = self.tx.clone();
        let max_turns = self.max_turns;

        tokio::spawn(async move {
            let final_text = match backend
                .stream_text(prompt, history, max_turns, |chunk| {
                    tx.send(RuntimeEvent::Chunk(chunk.to_string()))
                        .map_err(|e| CliError::operation_failed("chat stream", e.to_string()))?;
                    Ok(())
                })
                .await
            {
                Ok(final_text) => final_text,
                Err(error) => {
                    let _ = tx.send(RuntimeEvent::Failed(error.to_string()));
                    return;
                }
            };

            let _ = tx.send(RuntimeEvent::Completed(final_text));
        });
    }
}
