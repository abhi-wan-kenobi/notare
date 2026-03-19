use sqlx::SqlitePool;

use crate::SessionRow;

pub async fn insert_session(
    pool: &SqlitePool,
    session_id: &str,
    event_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO sessions (id, event_id) VALUES (?, ?)")
        .bind(session_id)
        .bind(event_id)
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

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, Option<String>, Option<String>)>(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM sessions ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, title, user_id, visibility, folder_id, event_id)| SessionRow {
                id,
                created_at,
                title,
                user_id,
                visibility,
                folder_id,
                event_id,
            },
        )
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
            String,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT id, created_at, title, user_id, visibility, folder_id, event_id FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, created_at, title, user_id, visibility, folder_id, event_id)| SessionRow {
            id,
            created_at,
            title,
            user_id,
            visibility,
            folder_id,
            event_id,
        },
    ))
}
