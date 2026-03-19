use sqlx::SqlitePool;

pub async fn set_session_visibility(
    pool: &SqlitePool,
    session_id: &str,
    visibility: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE sessions SET visibility = ? WHERE id = ?")
        .bind(visibility)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE words SET visibility = ? WHERE session_id = ?")
        .bind(visibility)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE speaker_hints SET visibility = ? WHERE session_id = ?")
        .bind(visibility)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE notes SET visibility = ? WHERE session_id = ?")
        .bind(visibility)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
