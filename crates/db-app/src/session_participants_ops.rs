use sqlx::SqlitePool;

use crate::SessionParticipantRow;

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
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, session_id, human_id, source, user_id FROM session_participants WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, session_id, human_id, source, user_id)| SessionParticipantRow {
                id,
                session_id,
                human_id,
                source,
                user_id,
            },
        )
        .collect())
}

pub async fn list_sessions_by_human(
    pool: &SqlitePool,
    human_id: &str,
) -> Result<Vec<SessionParticipantRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, session_id, human_id, source, user_id FROM session_participants WHERE human_id = ?",
    )
    .bind(human_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, session_id, human_id, source, user_id)| SessionParticipantRow {
                id,
                session_id,
                human_id,
                source,
                user_id,
            },
        )
        .collect())
}
