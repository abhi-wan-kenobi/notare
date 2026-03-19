use sqlx::SqlitePool;

use crate::ChatMessageRow;

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
