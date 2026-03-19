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

pub async fn copy_event_participants_to_session(
    pool: &SqlitePool,
    session_id: &str,
    event_id: &str,
) -> Result<usize, sqlx::Error> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO session_participants (id, session_id, human_id, source) SELECT ? || ':' || human_id, ?, human_id, 'event' FROM event_participants WHERE event_id = ? AND human_id IS NOT NULL",
    )
    .bind(session_id)
    .bind(session_id)
    .bind(event_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as usize)
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
