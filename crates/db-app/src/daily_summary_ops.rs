use sqlx::{Row, SqlitePool};

use crate::{DailySummaryRow, UpsertDailySummary};

pub async fn get_daily_summary(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<DailySummaryRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, daily_note_id, date, content, timeline_json, topics_json, status, \
                source_cursor_ms, source_fingerprint, generation_error, generated_at, \
                created_at, updated_at \
         FROM daily_summaries WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_summary_row))
}

pub async fn get_daily_summary_by_date(
    pool: &SqlitePool,
    date: &str,
    daily_note_id: &str,
) -> Result<Option<DailySummaryRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, daily_note_id, date, content, timeline_json, topics_json, status, \
                source_cursor_ms, source_fingerprint, generation_error, generated_at, \
                created_at, updated_at \
         FROM daily_summaries WHERE date = ? AND daily_note_id = ?",
    )
    .bind(date)
    .bind(daily_note_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.as_ref().map(map_daily_summary_row))
}

pub async fn upsert_daily_summary(
    pool: &SqlitePool,
    input: UpsertDailySummary<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO daily_summaries ( \
            id, daily_note_id, date, content, timeline_json, topics_json, status, \
            source_cursor_ms, source_fingerprint, generation_error, generated_at, updated_at \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(daily_note_id) DO UPDATE SET \
            date = excluded.date, \
            content = excluded.content, \
            timeline_json = excluded.timeline_json, \
            topics_json = excluded.topics_json, \
            status = excluded.status, \
            source_cursor_ms = excluded.source_cursor_ms, \
            source_fingerprint = excluded.source_fingerprint, \
            generation_error = excluded.generation_error, \
            generated_at = excluded.generated_at, \
            updated_at = excluded.updated_at",
    )
    .bind(input.id)
    .bind(input.daily_note_id)
    .bind(input.date)
    .bind(input.content)
    .bind(input.timeline_json)
    .bind(input.topics_json)
    .bind(input.status)
    .bind(input.source_cursor_ms)
    .bind(input.source_fingerprint)
    .bind(input.generation_error)
    .bind(input.generated_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn mark_daily_summary_generating(
    pool: &SqlitePool,
    id: &str,
    daily_note_id: &str,
    date: &str,
) -> Result<(), sqlx::Error> {
    upsert_daily_summary(
        pool,
        UpsertDailySummary {
            id,
            daily_note_id,
            date,
            content: "",
            timeline_json: "[]",
            topics_json: "[]",
            status: "generating",
            source_cursor_ms: 0,
            source_fingerprint: "",
            generation_error: "",
            generated_at: "",
        },
    )
    .await
}

pub async fn mark_daily_summary_error(
    pool: &SqlitePool,
    id: &str,
    daily_note_id: &str,
    date: &str,
    generation_error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO daily_summaries ( \
            id, daily_note_id, date, status, generation_error, updated_at \
         ) VALUES (?, ?, ?, 'error', ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
         ON CONFLICT(daily_note_id) DO UPDATE SET \
            status = 'error', \
            generation_error = excluded.generation_error, \
            updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(daily_note_id)
    .bind(date)
    .bind(generation_error)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_daily_summary(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM daily_summaries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

fn map_daily_summary_row(row: &sqlx::sqlite::SqliteRow) -> DailySummaryRow {
    DailySummaryRow {
        id: row.get("id"),
        daily_note_id: row.get("daily_note_id"),
        date: row.get("date"),
        content: row.get("content"),
        timeline_json: row.get("timeline_json"),
        topics_json: row.get("topics_json"),
        status: row.get("status"),
        source_cursor_ms: row.get("source_cursor_ms"),
        source_fingerprint: row.get("source_fingerprint"),
        generation_error: row.get("generation_error"),
        generated_at: row.get("generated_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
