use sqlx::{Row, SqlitePool};

use crate::DailyNoteRow;

pub async fn get_daily_note(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<DailyNoteRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, date, body, user_id, visibility, created_at, updated_at \
         FROM daily_notes WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_note_row))
}

pub async fn get_daily_note_by_date(
    pool: &SqlitePool,
    date: &str,
    user_id: &str,
) -> Result<Option<DailyNoteRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, date, body, user_id, visibility, created_at, updated_at \
         FROM daily_notes WHERE date = ? AND user_id = ?",
    )
    .bind(date)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_note_row))
}

pub async fn get_or_create_daily_note(
    pool: &SqlitePool,
    id: &str,
    date: &str,
    user_id: &str,
) -> Result<DailyNoteRow, sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO daily_notes (id, date, user_id) VALUES (?, ?, ?)")
        .bind(id)
        .bind(date)
        .bind(user_id)
        .execute(pool)
        .await?;

    get_daily_note_by_date(pool, date, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn upsert_daily_note_body(
    pool: &SqlitePool,
    id: &str,
    date: &str,
    body: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO daily_notes (id, date, body, user_id, updated_at) \
         VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           date = excluded.date, \
           body = excluded.body, \
           user_id = excluded.user_id, \
           updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(date)
    .bind(body)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_daily_notes_in_range(
    pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
    user_id: &str,
) -> Result<Vec<DailyNoteRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, date, body, user_id, visibility, created_at, updated_at \
         FROM daily_notes \
         WHERE date >= ? AND date <= ? AND user_id = ? \
         ORDER BY date",
    )
    .bind(start_date)
    .bind(end_date)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(map_daily_note_row).collect())
}

fn map_daily_note_row(row: &sqlx::sqlite::SqliteRow) -> DailyNoteRow {
    DailyNoteRow {
        id: row.get("id"),
        date: row.get("date"),
        body: row.get("body"),
        user_id: row.get("user_id"),
        visibility: row.get("visibility"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
