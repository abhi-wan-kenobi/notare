use std::path::PathBuf;

use tokio::sync::mpsc;

use hypr_db_app::{PersistableSpeakerHint, TranscriptDeltaPersist};
use hypr_transcript::{FinalizedWord, RuntimeSpeakerHint, WordRef};

use crate::llm::ResolvedLlmConfig;

pub use super::super::exit::{AUTO_EXIT_DELAY, ExitEvent, ExitScreen};

fn segments_to_markdown(segments: &[hypr_transcript::Segment]) -> String {
    use hypr_transcript::SpeakerLabeler;

    let mut labeler = SpeakerLabeler::from_segments(segments, None);
    let mut out = String::new();

    for segment in segments {
        let speaker = labeler.label_for(&segment.key, None);
        let start_secs = segment
            .words
            .first()
            .map(|w| w.start_ms / 1000)
            .unwrap_or(0);
        let mm = start_secs / 60;
        let ss = start_secs % 60;

        out.push_str(&format!("**{speaker}** ({mm:02}:{ss:02})\n"));

        let text: String = segment
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&text);
        out.push_str("\n\n");
    }

    out
}

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

pub fn spawn_post_session(
    segments: Vec<hypr_transcript::Segment>,
    session_dir: PathBuf,
    llm_config: Result<ResolvedLlmConfig, String>,
    tx: mpsc::UnboundedSender<ExitEvent>,
    words: Vec<FinalizedWord>,
    hints: Vec<RuntimeSpeakerHint>,
    memo_text: String,
    session_id: String,
    db_path: PathBuf,
) {
    tokio::spawn(async move {
        // Task 0: save to database
        let _ = tx.send(ExitEvent::TaskStarted(0));
        let pool = match hypr_db_core2::Db3::connect_local_plain(&db_path).await {
            Ok(db) => match hypr_db_app::migrate(db.pool()).await {
                Ok(()) => {
                    let pool = db.pool().clone();
                    if let Err(e) = hypr_db_app::insert_session(&pool, &session_id).await {
                        let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                        None
                    } else {
                        let delta = TranscriptDeltaPersist {
                            new_words: words,
                            hints: to_persistable_hints(&hints),
                            replaced_ids: vec![],
                        };
                        if let Err(e) = hypr_db_app::apply_delta(&pool, &session_id, &delta).await {
                            let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                            None
                        } else {
                            let memo = memo_text.trim();
                            if !memo.is_empty() {
                                let _ = hypr_db_app::update_session(
                                    &pool,
                                    &session_id,
                                    None,
                                    None,
                                    Some(memo),
                                )
                                .await;
                            }
                            let _ = tx.send(ExitEvent::TaskDone(0));
                            Some(pool)
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                    None
                }
            },
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(0, e.to_string()));
                None
            }
        };

        // Task 1: save transcript
        let _ = tx.send(ExitEvent::TaskStarted(1));
        let markdown = segments_to_markdown(&segments);
        let transcript_path = session_dir.join("transcript.md");
        match hypr_storage::fs::atomic_write_async(&transcript_path, &markdown).await {
            Ok(()) => {
                let _ = tx.send(ExitEvent::TaskDone(1));
            }
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        }

        // Task 2: generate summary
        let _ = tx.send(ExitEvent::TaskStarted(2));
        let config = match llm_config {
            Ok(config) => config,
            Err(msg) => {
                let _ = tx.send(ExitEvent::TaskFailed(2, msg));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        };

        let backend = match crate::agent::Backend::new(config, None) {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(2, e.to_string()));
                let _ = tx.send(ExitEvent::AllDone);
                tokio::time::sleep(AUTO_EXIT_DELAY).await;
                let _ = tx.send(ExitEvent::AutoExit);
                return;
            }
        };

        let prompt = format!(
            "Summarize the following meeting transcript in a few concise paragraphs. \
             Focus on key topics, decisions, and action items.\n\n{markdown}"
        );

        match backend
            .stream_text(prompt, vec![], 1, |_chunk| Ok(()))
            .await
        {
            Ok(Some(summary)) => {
                let summary_path = session_dir.join("summary.md");
                match hypr_storage::fs::atomic_write_async(&summary_path, &summary).await {
                    Ok(()) => {
                        let _ = tx.send(ExitEvent::TaskDone(2));
                    }
                    Err(e) => {
                        let _ = tx.send(ExitEvent::TaskFailed(2, e.to_string()));
                    }
                }
                if let Some(pool) = &pool {
                    let title = title_from_summary(&summary);
                    let _ = hypr_db_app::update_session(
                        pool,
                        &session_id,
                        Some(&title),
                        Some(&summary),
                        None,
                    )
                    .await;
                }
            }
            Ok(None) => {
                let _ = tx.send(ExitEvent::TaskFailed(
                    2,
                    "LLM returned empty response".into(),
                ));
            }
            Err(e) => {
                let _ = tx.send(ExitEvent::TaskFailed(2, e.to_string()));
            }
        }

        let _ = tx.send(ExitEvent::AllDone);
        tokio::time::sleep(AUTO_EXIT_DELAY).await;
        let _ = tx.send(ExitEvent::AutoExit);
    });
}
