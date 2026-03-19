use sqlx::SqlitePool;
use tokio::sync::mpsc;

use hypr_db_app::{PersistableSpeakerHint, TranscriptDeltaPersist};
use hypr_transcript::{FinalizedWord, RuntimeSpeakerHint, WordRef};

use crate::llm::ResolvedLlmConfig;

pub use super::super::exit::{AUTO_EXIT_DELAY, ExitEvent, ExitScreen};

fn to_persistable_hints(hints: &[RuntimeSpeakerHint]) -> Vec<PersistableSpeakerHint> {
    hints
        .iter()
        .filter_map(|hint| match &hint.target {
            WordRef::FinalWordId(word_id) => Some(PersistableSpeakerHint {
                word_id: word_id.clone(),
                data: hint.data.clone(),
            }),
            WordRef::RuntimeIndex(_) => None,
        })
        .collect()
}

fn title_from_summary(summary: &str) -> String {
    let first_sentence = summary
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or(summary);
    let trimmed = first_sentence.trim();
    if trimmed.len() <= 80 {
        trimmed.to_string()
    } else {
        let mut end = 80;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &trimmed[..end])
    }
}

fn words_to_transcript_text(words: &[FinalizedWord]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn spawn_post_session(
    llm_config: Result<ResolvedLlmConfig, String>,
    tx: mpsc::UnboundedSender<ExitEvent>,
    words: Vec<FinalizedWord>,
    hints: Vec<RuntimeSpeakerHint>,
    memo_text: String,
    session_id: String,
    pool: SqlitePool,
) {
    tokio::spawn(async move {
        // Task 0: save to database
        let _ = tx.send(ExitEvent::TaskStarted(0));
        let ok = match hypr_db_app::insert_session(&pool, &session_id).await {
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                false
            }
            Ok(()) => {
                let delta = TranscriptDeltaPersist {
                    new_words: words,
                    hints: to_persistable_hints(&hints),
                    replaced_ids: vec![],
                };
                match hypr_db_app::apply_delta(&pool, &session_id, &delta).await {
                    Err(e) => {
                        let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                        false
                    }
                    Ok(()) => {
                        let memo = memo_text.trim();
                        if !memo.is_empty() {
                            let note_id = format!("{session_id}:memo");
                            let _ = hypr_db_app::insert_note(
                                &pool,
                                &note_id,
                                &session_id,
                                "memo",
                                "",
                                memo,
                            )
                            .await;
                        }
                        let _ = tx.send(ExitEvent::TaskDone(0));
                        true
                    }
                }
            }
        };

        // Task 1: generate summary
        if !ok {
            let _ = tx.send(ExitEvent::TaskFailed(1, "database unavailable".into()));
            let _ = tx.send(ExitEvent::AllDone);
            tokio::time::sleep(AUTO_EXIT_DELAY).await;
            let _ = tx.send(ExitEvent::AutoExit);
            return;
        }

        let _ = tx.send(ExitEvent::TaskStarted(1));

        let transcript_text = match hypr_db_app::load_words(&pool, &session_id).await {
            Ok(words) => words_to_transcript_text(&words),
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        };

        let config = match llm_config {
            Ok(config) => config,
            Err(msg) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, msg));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        };

        let backend = match crate::agent::Backend::new(config, None) {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        };

        let prompt = format!(
            "Summarize the following meeting transcript in a few concise paragraphs. \
             Focus on key topics, decisions, and action items.\n\n{transcript_text}"
        );

        match backend
            .stream_text(prompt, vec![], 1, |_chunk| Ok(()))
            .await
        {
            Ok(Some(summary)) => {
                let _ = tx.send(ExitEvent::TaskDone(1));
                let title = title_from_summary(&summary);
                let _ = hypr_db_app::update_session(&pool, &session_id, Some(&title)).await;
                let note_id = format!("{session_id}:summary");
                let _ =
                    hypr_db_app::insert_note(&pool, &note_id, &session_id, "summary", "", &summary)
                        .await;
            }
            Ok(None) => {
                let _ = tx.send(ExitEvent::TaskFailed(
                    1,
                    "LLM returned empty response".into(),
                ));
            }
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
            }
        }

        let _ = tx.send(ExitEvent::AllDone);
        tokio::time::sleep(AUTO_EXIT_DELAY).await;
        let _ = tx.send(ExitEvent::AutoExit);
    });
}
