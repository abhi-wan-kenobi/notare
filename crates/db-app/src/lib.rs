#![forbid(unsafe_code)]

use hypr_transcript::{FinalizedWord, SpeakerHintData, WordState};
use sqlx::SqlitePool;

pub struct ChatMessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub struct SessionRow {
    pub id: String,
    pub created_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub memo: Option<String>,
}

pub struct TranscriptDeltaPersist {
    pub new_words: Vec<FinalizedWord>,
    pub hints: Vec<PersistableSpeakerHint>,
    pub replaced_ids: Vec<String>,
}

pub struct PersistableSpeakerHint {
    pub word_id: String,
    pub data: SpeakerHintData,
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub async fn insert_session(pool: &SqlitePool, session_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO sessions (id) VALUES (?)")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_session(
    pool: &SqlitePool,
    session_id: &str,
    title: Option<&str>,
    summary: Option<&str>,
    memo: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions SET title = COALESCE(?, title), summary = COALESCE(?, summary), memo = COALESCE(?, memo) WHERE id = ?",
    )
    .bind(title)
    .bind(summary)
    .bind(memo)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn apply_delta(
    pool: &SqlitePool,
    session_id: &str,
    delta: &TranscriptDeltaPersist,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for id in &delta.replaced_ids {
        sqlx::query("DELETE FROM speaker_hints WHERE word_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM words WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    for w in &delta.new_words {
        let state_str = match w.state {
            WordState::Final => "final",
            WordState::Pending => "pending",
        };
        sqlx::query(
            "INSERT OR REPLACE INTO words (id, session_id, text, start_ms, end_ms, channel, state) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&w.id)
        .bind(session_id)
        .bind(&w.text)
        .bind(w.start_ms)
        .bind(w.end_ms)
        .bind(w.channel)
        .bind(state_str)
        .execute(&mut *tx)
        .await?;
    }

    for h in &delta.hints {
        let (kind, speaker_index, provider, channel, human_id) = match &h.data {
            SpeakerHintData::ProviderSpeakerIndex {
                speaker_index,
                provider,
                channel,
            } => (
                "provider_speaker_index",
                Some(*speaker_index),
                provider.as_deref(),
                *channel,
                None,
            ),
            SpeakerHintData::UserSpeakerAssignment { human_id } => (
                "user_speaker_assignment",
                None,
                None,
                None,
                Some(human_id.as_str()),
            ),
        };
        let hint_id = format!("{session_id}:{}:{kind}", h.word_id);
        sqlx::query(
            "INSERT OR REPLACE INTO speaker_hints (id, session_id, word_id, kind, speaker_index, provider, channel, human_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&hint_id)
        .bind(session_id)
        .bind(&h.word_id)
        .bind(kind)
        .bind(speaker_index)
        .bind(provider)
        .bind(channel)
        .bind(human_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn load_words(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<FinalizedWord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, i32, String)>(
        "SELECT id, text, start_ms, end_ms, channel, state FROM words WHERE session_id = ? ORDER BY start_ms",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, text, start_ms, end_ms, channel, state)| {
            let state = match state.as_str() {
                "pending" => WordState::Pending,
                _ => WordState::Final,
            };
            FinalizedWord {
                id,
                text,
                start_ms,
                end_ms,
                channel,
                state,
            }
        })
        .collect())
}

pub async fn load_hints(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<PersistableSpeakerHint>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<i32>, Option<String>, Option<i32>, Option<String>)>(
        "SELECT word_id, kind, speaker_index, provider, channel, human_id FROM speaker_hints WHERE session_id = ? ORDER BY word_id",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(word_id, kind, speaker_index, provider, channel, human_id)| {
                let data = match kind.as_str() {
                    "provider_speaker_index" => SpeakerHintData::ProviderSpeakerIndex {
                        speaker_index: speaker_index.unwrap_or(0),
                        provider,
                        channel,
                    },
                    "user_speaker_assignment" => SpeakerHintData::UserSpeakerAssignment {
                        human_id: human_id.unwrap_or_default(),
                    },
                    _ => SpeakerHintData::ProviderSpeakerIndex {
                        speaker_index: speaker_index.unwrap_or(0),
                        provider,
                        channel,
                    },
                };
                PersistableSpeakerHint { word_id, data }
            },
        )
        .collect())
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT id, created_at, title, summary, memo FROM sessions ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, created_at, title, summary, memo)| SessionRow {
            id,
            created_at,
            title,
            summary,
            memo,
        })
        .collect())
}

pub async fn get_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<SessionRow>, sqlx::Error> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >("SELECT id, created_at, title, summary, memo FROM sessions WHERE id = ?")
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(
        row.map(|(id, created_at, title, summary, memo)| SessionRow {
            id,
            created_at,
            title,
            summary,
            memo,
        }),
    )
}

pub async fn insert_chat_message(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO chat_messages (id, session_id, role, content) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_chat_messages(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<ChatMessageRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, session_id, role, content, created_at FROM chat_messages WHERE session_id = ? ORDER BY created_at",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, session_id, role, content, created_at)| ChatMessageRow {
                id,
                session_id,
                role,
                content,
                created_at,
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    #[tokio::test]
    async fn roundtrip_words_and_hints() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "sess-1";
        insert_session(db.pool(), sid).await.unwrap();

        let session = get_session(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(session.id, sid);
        assert!(session.title.is_none());

        update_session(db.pool(), sid, Some("My Title"), None, Some("a memo"))
            .await
            .unwrap();
        let session = get_session(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(session.title.as_deref(), Some("My Title"));
        assert_eq!(session.memo.as_deref(), Some("a memo"));
        assert!(session.summary.is_none());

        let delta = TranscriptDeltaPersist {
            new_words: vec![
                FinalizedWord {
                    id: "w1".into(),
                    text: "hello".into(),
                    start_ms: 0,
                    end_ms: 500,
                    channel: 0,
                    state: WordState::Final,
                },
                FinalizedWord {
                    id: "w2".into(),
                    text: "world".into(),
                    start_ms: 500,
                    end_ms: 1000,
                    channel: 0,
                    state: WordState::Pending,
                },
            ],
            hints: vec![PersistableSpeakerHint {
                word_id: "w1".into(),
                data: SpeakerHintData::ProviderSpeakerIndex {
                    speaker_index: 0,
                    provider: Some("deepgram".into()),
                    channel: Some(0),
                },
            }],
            replaced_ids: vec![],
        };
        apply_delta(db.pool(), sid, &delta).await.unwrap();

        let words = load_words(db.pool(), sid).await.unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].id, "w1");
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[1].state, WordState::Pending);

        let hints = load_hints(db.pool(), sid).await.unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].word_id, "w1");
        match &hints[0].data {
            SpeakerHintData::ProviderSpeakerIndex {
                speaker_index,
                provider,
                ..
            } => {
                assert_eq!(*speaker_index, 0);
                assert_eq!(provider.as_deref(), Some("deepgram"));
            }
            _ => panic!("expected ProviderSpeakerIndex"),
        }
    }

    #[tokio::test]
    async fn replacement_removes_old_words() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "sess-2";
        insert_session(db.pool(), sid).await.unwrap();

        let delta1 = TranscriptDeltaPersist {
            new_words: vec![FinalizedWord {
                id: "w1".into(),
                text: "helo".into(),
                start_ms: 0,
                end_ms: 500,
                channel: 0,
                state: WordState::Pending,
            }],
            hints: vec![PersistableSpeakerHint {
                word_id: "w1".into(),
                data: SpeakerHintData::UserSpeakerAssignment {
                    human_id: "user-a".into(),
                },
            }],
            replaced_ids: vec![],
        };
        apply_delta(db.pool(), sid, &delta1).await.unwrap();

        let delta2 = TranscriptDeltaPersist {
            new_words: vec![FinalizedWord {
                id: "w1-corrected".into(),
                text: "hello".into(),
                start_ms: 0,
                end_ms: 500,
                channel: 0,
                state: WordState::Final,
            }],
            hints: vec![],
            replaced_ids: vec!["w1".into()],
        };
        apply_delta(db.pool(), sid, &delta2).await.unwrap();

        let words = load_words(db.pool(), sid).await.unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].id, "w1-corrected");
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[0].state, WordState::Final);

        let hints = load_hints(db.pool(), sid).await.unwrap();
        assert!(hints.is_empty());
    }

    #[tokio::test]
    async fn chat_message_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "chat-sess-1";
        insert_session(db.pool(), sid).await.unwrap();

        insert_chat_message(db.pool(), "m1", sid, "user", "hello")
            .await
            .unwrap();
        insert_chat_message(db.pool(), "m2", sid, "assistant", "hi there")
            .await
            .unwrap();

        let messages = load_chat_messages(db.pool(), sid).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, "m1");
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].id, "m2");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "hi there");
    }
}
