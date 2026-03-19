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
}

pub struct NoteRow {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

pub struct HumanRow {
    pub id: String,
    pub created_at: String,
    pub name: String,
    pub email: String,
    pub org_id: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: i32,
}

pub struct OrganizationRow {
    pub id: String,
    pub created_at: String,
    pub name: String,
    pub pinned: bool,
    pub pin_order: i32,
}

pub struct SessionParticipantRow {
    pub id: String,
    pub session_id: String,
    pub human_id: String,
    pub source: String,
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
    sqlx::query("INSERT OR IGNORE INTO sessions (id) VALUES (?)")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_session(
    pool: &SqlitePool,
    session_id: &str,
    title: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET title = COALESCE(?, title) WHERE id = ?")
        .bind(title)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_note(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    kind: &str,
    title: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO notes (id, session_id, kind, title, content) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(session_id)
        .bind(kind)
        .bind(title)
        .bind(content)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_notes_by_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<NoteRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, session_id, kind, title, content, created_at FROM notes WHERE session_id = ? ORDER BY created_at",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, session_id, kind, title, content, created_at)| NoteRow {
                id,
                session_id,
                kind,
                title,
                content,
                created_at,
            },
        )
        .collect())
}

pub async fn get_note_by_session_and_kind(
    pool: &SqlitePool,
    session_id: &str,
    kind: &str,
) -> Result<Option<NoteRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, session_id, kind, title, content, created_at FROM notes WHERE session_id = ? AND kind = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .bind(kind)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, session_id, kind, title, content, created_at)| NoteRow {
            id,
            session_id,
            kind,
            title,
            content,
            created_at,
        },
    ))
}

pub async fn update_note(pool: &SqlitePool, id: &str, content: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notes SET content = ? WHERE id = ?")
        .bind(content)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_notes_by_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM notes WHERE session_id = ?")
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
        .filter_map(
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
                    _ => return None,
                };
                Some(PersistableSpeakerHint { word_id, data })
            },
        )
        .collect())
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, created_at, title FROM sessions ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, created_at, title)| SessionRow {
            id,
            created_at,
            title,
        })
        .collect())
}

pub async fn get_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<SessionRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, created_at, title FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, created_at, title)| SessionRow {
        id,
        created_at,
        title,
    }))
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

// --- Humans ---

pub async fn insert_human(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    email: &str,
    org_id: &str,
    job_title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO humans (id, name, email, org_id, job_title) VALUES (?, ?, ?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(email)
        .bind(org_id)
        .bind(job_title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_human(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    email: Option<&str>,
    org_id: Option<&str>,
    job_title: Option<&str>,
    memo: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE humans SET name = COALESCE(?, name), email = COALESCE(?, email), org_id = COALESCE(?, org_id), job_title = COALESCE(?, job_title), memo = COALESCE(?, memo) WHERE id = ?",
    )
    .bind(name)
    .bind(email)
    .bind(org_id)
    .bind(job_title)
    .bind(memo)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_human(pool: &SqlitePool, id: &str) -> Result<Option<HumanRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order FROM humans WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            created_at,
            name,
            email,
            org_id,
            job_title,
            linkedin_username,
            memo,
            pinned,
            pin_order,
        )| HumanRow {
            id,
            created_at,
            name,
            email,
            org_id,
            job_title,
            linkedin_username,
            memo,
            pinned: pinned != 0,
            pin_order,
        },
    ))
}

pub async fn list_humans(pool: &SqlitePool) -> Result<Vec<HumanRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order FROM humans ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned,
                pin_order,
            )| HumanRow {
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned: pinned != 0,
                pin_order,
            },
        )
        .collect())
}

pub async fn list_humans_by_org(
    pool: &SqlitePool,
    org_id: &str,
) -> Result<Vec<HumanRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, i32, i32)>(
        "SELECT id, created_at, name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order FROM humans WHERE org_id = ? ORDER BY created_at DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned,
                pin_order,
            )| HumanRow {
                id,
                created_at,
                name,
                email,
                org_id,
                job_title,
                linkedin_username,
                memo,
                pinned: pinned != 0,
                pin_order,
            },
        )
        .collect())
}

pub async fn delete_human(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM session_participants WHERE human_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM humans WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// --- Organizations ---

pub async fn insert_organization(
    pool: &SqlitePool,
    id: &str,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO organizations (id, name) VALUES (?, ?)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_organization(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE organizations SET name = COALESCE(?, name) WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_organization(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<OrganizationRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, i32, i32)>(
        "SELECT id, created_at, name, pinned, pin_order FROM organizations WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, created_at, name, pinned, pin_order)| OrganizationRow {
            id,
            created_at,
            name,
            pinned: pinned != 0,
            pin_order,
        },
    ))
}

pub async fn list_organizations(pool: &SqlitePool) -> Result<Vec<OrganizationRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, i32)>(
        "SELECT id, created_at, name, pinned, pin_order FROM organizations ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, name, pinned, pin_order)| OrganizationRow {
                id,
                created_at,
                name,
                pinned: pinned != 0,
                pin_order,
            },
        )
        .collect())
}

pub async fn delete_organization(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE humans SET org_id = '' WHERE org_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM organizations WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// --- Session Participants ---

pub async fn add_session_participant(
    pool: &SqlitePool,
    session_id: &str,
    human_id: &str,
    source: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{session_id}:{human_id}");
    sqlx::query(
        "INSERT OR REPLACE INTO session_participants (id, session_id, human_id, source) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(human_id)
    .bind(source)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_session_participant(
    pool: &SqlitePool,
    session_id: &str,
    human_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM session_participants WHERE session_id = ? AND human_id = ?")
        .bind(session_id)
        .bind(human_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_session_participants(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<SessionParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, session_id, human_id, source FROM session_participants WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, human_id, source)| SessionParticipantRow {
            id,
            session_id,
            human_id,
            source,
        })
        .collect())
}

// --- Connections ---

pub struct ConnectionRow {
    pub id: String,
    pub provider_type: String,
    pub provider_id: String,
    pub base_url: String,
    pub api_key: String,
}

pub async fn upsert_connection(
    pool: &SqlitePool,
    provider_type: &str,
    provider_id: &str,
    base_url: &str,
    api_key: &str,
) -> Result<(), sqlx::Error> {
    let id = format!("{provider_type}:{provider_id}");
    sqlx::query(
        "INSERT OR REPLACE INTO connections (id, provider_type, provider_id, base_url, api_key) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(provider_type)
    .bind(provider_id)
    .bind(base_url)
    .bind(api_key)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_connection(
    pool: &SqlitePool,
    provider_type: &str,
    provider_id: &str,
) -> Result<Option<ConnectionRow>, sqlx::Error> {
    let id = format!("{provider_type}:{provider_id}");
    let row = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, provider_type, provider_id, base_url, api_key FROM connections WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, provider_type, provider_id, base_url, api_key)| ConnectionRow {
            id,
            provider_type,
            provider_id,
            base_url,
            api_key,
        },
    ))
}

pub async fn list_connections(
    pool: &SqlitePool,
    provider_type: &str,
) -> Result<Vec<ConnectionRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, provider_type, provider_id, base_url, api_key FROM connections WHERE provider_type = ? ORDER BY provider_id",
    )
    .bind(provider_type)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, provider_type, provider_id, base_url, api_key)| ConnectionRow {
                id,
                provider_type,
                provider_id,
                base_url,
                api_key,
            },
        )
        .collect())
}

pub async fn list_configured_provider_ids(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>("SELECT DISTINCT provider_id FROM connections")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

// --- Settings ---

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(v,)| v))
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_all_settings(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn list_sessions_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<SessionParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, session_id, human_id, source FROM session_participants WHERE human_id = ?",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, human_id, source)| SessionParticipantRow {
            id,
            session_id,
            human_id,
            source,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    // https://docs.sqlitecloud.io/docs/sqlite-sync-best-practices
    mod sync_compat {
        use super::*;

        // PRAGMA table_info returns: (cid, name, type, notnull, dflt_value, pk)
        type PragmaRow = (i32, String, String, i32, Option<String>, i32);

        async fn table_names(pool: &sqlx::SqlitePool) -> Vec<String> {
            sqlx::query_as::<_, (String,)>(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' AND name NOT LIKE '%_fts%'",
            )
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.0)
            .collect()
        }

        async fn table_info(pool: &sqlx::SqlitePool, table: &str) -> Vec<PragmaRow> {
            sqlx::query_as::<_, PragmaRow>(&format!("PRAGMA table_info('{}')", table))
                .fetch_all(pool)
                .await
                .unwrap()
        }

        #[tokio::test]
        async fn primary_keys_are_text_not_null() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            for table in &table_names(db.pool()).await {
                let cols = table_info(db.pool(), table).await;
                let pks: Vec<_> = cols.iter().filter(|c| c.5 != 0).collect();
                assert!(!pks.is_empty(), "{table}: no primary key");
                for pk in &pks {
                    assert_eq!(
                        pk.2.to_uppercase(),
                        "TEXT",
                        "{table}.{}: pk must be TEXT, got {}",
                        pk.1,
                        pk.2
                    );
                    assert_ne!(pk.3, 0, "{table}.{}: pk must be NOT NULL", pk.1);
                }
            }
        }

        #[tokio::test]
        async fn not_null_columns_have_defaults() {
            let db = Db3::connect_memory_plain().await.unwrap();
            migrate(db.pool()).await.unwrap();

            let mut violations = vec![];
            for table in &table_names(db.pool()).await {
                for col in &table_info(db.pool(), table).await {
                    let (_, ref name, _, notnull, ref dflt, pk) = *col;
                    if pk != 0 || notnull == 0 {
                        continue;
                    }
                    if dflt.is_none() {
                        violations.push(format!("{table}.{name}"));
                    }
                }
            }

            assert!(
                violations.is_empty(),
                "NOT NULL non-PK columns without DEFAULT: {}",
                violations.join(", ")
            );
        }
    }

    #[tokio::test]
    async fn roundtrip_words_and_hints() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "sess-1";
        insert_session(db.pool(), sid).await.unwrap();

        let session = get_session(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(session.id, sid);
        assert!(session.title.is_none());

        update_session(db.pool(), sid, Some("My Title"))
            .await
            .unwrap();
        let session = get_session(db.pool(), sid).await.unwrap().unwrap();
        assert_eq!(session.title.as_deref(), Some("My Title"));

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

    #[tokio::test]
    async fn human_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_human(
            db.pool(),
            "h1",
            "Alice",
            "alice@example.com",
            "",
            "Engineer",
        )
        .await
        .unwrap();

        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.name, "Alice");
        assert_eq!(human.email, "alice@example.com");
        assert_eq!(human.job_title, "Engineer");

        update_human(
            db.pool(),
            "h1",
            Some("Alice B"),
            None,
            None,
            None,
            Some("notes"),
        )
        .await
        .unwrap();
        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.name, "Alice B");
        assert_eq!(human.email, "alice@example.com");
        assert_eq!(human.memo, "notes");

        let all = list_humans(db.pool()).await.unwrap();
        assert_eq!(all.len(), 1);

        delete_human(db.pool(), "h1").await.unwrap();
        assert!(get_human(db.pool(), "h1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn organization_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_organization(db.pool(), "org1", "Acme")
            .await
            .unwrap();

        let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
        assert_eq!(org.name, "Acme");

        update_organization(db.pool(), "org1", Some("Acme Inc"))
            .await
            .unwrap();
        let org = get_organization(db.pool(), "org1").await.unwrap().unwrap();
        assert_eq!(org.name, "Acme Inc");

        insert_human(db.pool(), "h1", "Bob", "", "org1", "")
            .await
            .unwrap();

        delete_organization(db.pool(), "org1").await.unwrap();
        assert!(get_organization(db.pool(), "org1").await.unwrap().is_none());

        let human = get_human(db.pool(), "h1").await.unwrap().unwrap();
        assert_eq!(human.org_id, "");
    }

    #[tokio::test]
    async fn session_participant_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_session(db.pool(), "s1").await.unwrap();
        insert_human(db.pool(), "h1", "Alice", "", "", "")
            .await
            .unwrap();
        insert_human(db.pool(), "h2", "Bob", "", "", "")
            .await
            .unwrap();

        add_session_participant(db.pool(), "s1", "h1", "manual")
            .await
            .unwrap();
        add_session_participant(db.pool(), "s1", "h2", "auto")
            .await
            .unwrap();

        let participants = list_session_participants(db.pool(), "s1").await.unwrap();
        assert_eq!(participants.len(), 2);

        let sessions = list_sessions_by_human(db.pool(), "h1").await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");

        remove_session_participant(db.pool(), "s1", "h1")
            .await
            .unwrap();
        let participants = list_session_participants(db.pool(), "s1").await.unwrap();
        assert_eq!(participants.len(), 1);
        assert_eq!(participants[0].human_id, "h2");

        delete_human(db.pool(), "h2").await.unwrap();
        let participants = list_session_participants(db.pool(), "s1").await.unwrap();
        assert!(participants.is_empty());
    }

    #[tokio::test]
    async fn note_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let sid = "note-sess-1";
        insert_session(db.pool(), sid).await.unwrap();

        insert_note(db.pool(), "n1", sid, "memo", "", "my memo")
            .await
            .unwrap();
        insert_note(db.pool(), "n2", sid, "summary", "", "my summary")
            .await
            .unwrap();

        let notes = list_notes_by_session(db.pool(), sid).await.unwrap();
        assert_eq!(notes.len(), 2);

        let memo = get_note_by_session_and_kind(db.pool(), sid, "memo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(memo.content, "my memo");

        let summary = get_note_by_session_and_kind(db.pool(), sid, "summary")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(summary.content, "my summary");

        update_note(db.pool(), "n1", "updated memo").await.unwrap();
        let memo = get_note_by_session_and_kind(db.pool(), sid, "memo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(memo.content, "updated memo");

        delete_notes_by_session(db.pool(), sid).await.unwrap();
        let notes = list_notes_by_session(db.pool(), sid).await.unwrap();
        assert!(notes.is_empty());
    }
}
