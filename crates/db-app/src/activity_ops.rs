use sqlx::SqlitePool;

use crate::activity_types::{ObservationEventRow, PreferredObservationAnalysisRow};

pub struct InsertObservationEvent<'a> {
    pub id: &'a str,
    pub observation_id: &'a str,
    pub occurred_at_ms: i64,
    pub event_kind: &'a str,
    pub end_reason: Option<&'a str>,
    pub change_class: Option<&'a str>,
    pub app_id: &'a str,
    pub bundle_id: &'a str,
    pub app_name: &'a str,
    pub activity_kind: &'a str,
    pub window_title: &'a str,
    pub url: &'a str,
    pub domain: &'a str,
    pub text_anchor_identity: &'a str,
    pub observation_key: &'a str,
    pub snapshot_json: &'a str,
}

pub struct InsertScreenshot<'a> {
    pub id: &'a str,
    pub observation_id: &'a str,
    pub screenshot_kind: &'a str,
    pub scheduled_at_ms: i64,
    pub captured_at_ms: i64,
    pub app_name: &'a str,
    pub window_title: &'a str,
    pub mime_type: &'a str,
    pub width: i64,
    pub height: i64,
    pub sha256: &'a str,
    pub image_blob: &'a [u8],
    pub snapshot_json: &'a str,
}

pub struct InsertObservationAnalysis<'a> {
    pub id: &'a str,
    pub observation_id: &'a str,
    pub screenshot_id: &'a str,
    pub screenshot_kind: &'a str,
    pub captured_at_ms: i64,
    pub model_name: &'a str,
    pub prompt_version: &'a str,
    pub app_name: &'a str,
    pub window_title: &'a str,
    pub summary: &'a str,
}

pub async fn insert_observation_event(
    pool: &SqlitePool,
    input: InsertObservationEvent<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_observation_events ( \
            id, observation_id, occurred_at_ms, event_kind, end_reason, change_class, app_id, \
            bundle_id, app_name, activity_kind, window_title, url, domain, text_anchor_identity, \
            observation_key, snapshot_json \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.observation_id)
    .bind(input.occurred_at_ms)
    .bind(input.event_kind)
    .bind(input.end_reason)
    .bind(input.change_class)
    .bind(input.app_id)
    .bind(input.bundle_id)
    .bind(input.app_name)
    .bind(input.activity_kind)
    .bind(input.window_title)
    .bind(input.url)
    .bind(input.domain)
    .bind(input.text_anchor_identity)
    .bind(input.observation_key)
    .bind(input.snapshot_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_observation_events_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ObservationEventRow>, sqlx::Error> {
    sqlx::query_as::<_, ObservationEventRow>(
        "SELECT id, observation_id, occurred_at_ms, event_kind, end_reason, change_class, app_id, \
         bundle_id, app_name, activity_kind, window_title, url, domain, text_anchor_identity, \
         observation_key, snapshot_json, created_at \
         FROM activity_observation_events \
         WHERE occurred_at_ms >= ? AND occurred_at_ms < ? \
         ORDER BY occurred_at_ms, id",
    )
    .bind(start_ms)
    .bind(end_ms)
    .fetch_all(pool)
    .await
}

pub async fn insert_screenshot(
    pool: &SqlitePool,
    input: InsertScreenshot<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_screenshots ( \
            id, observation_id, screenshot_kind, scheduled_at_ms, captured_at_ms, app_name, \
            window_title, mime_type, width, height, sha256, image_blob, snapshot_json \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.observation_id)
    .bind(input.screenshot_kind)
    .bind(input.scheduled_at_ms)
    .bind(input.captured_at_ms)
    .bind(input.app_name)
    .bind(input.window_title)
    .bind(input.mime_type)
    .bind(input.width)
    .bind(input.height)
    .bind(input.sha256)
    .bind(input.image_blob)
    .bind(input.snapshot_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_observation_analysis(
    pool: &SqlitePool,
    input: InsertObservationAnalysis<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO activity_observation_analyses ( \
            id, observation_id, screenshot_id, screenshot_kind, captured_at_ms, model_name, \
            prompt_version, app_name, window_title, summary \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.observation_id)
    .bind(input.screenshot_id)
    .bind(input.screenshot_kind)
    .bind(input.captured_at_ms)
    .bind(input.model_name)
    .bind(input.prompt_version)
    .bind(input.app_name)
    .bind(input.window_title)
    .bind(input.summary)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_preferred_observation_analyses_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<PreferredObservationAnalysisRow>, sqlx::Error> {
    sqlx::query_as::<_, PreferredObservationAnalysisRow>(
        "WITH ranked AS ( \
           SELECT observation_id, screenshot_id, screenshot_kind, captured_at_ms, app_name, \
                  window_title, summary, \
                  ROW_NUMBER() OVER ( \
                    PARTITION BY observation_id \
                    ORDER BY CASE screenshot_kind \
                      WHEN 'settled' THEN 0 \
                      WHEN 'refresh' THEN 1 \
                      ELSE 2 \
                    END, \
                    captured_at_ms DESC, \
                    id DESC \
                  ) AS rank_in_observation \
           FROM activity_observation_analyses \
           WHERE captured_at_ms >= ? AND captured_at_ms < ? \
         ) \
         SELECT observation_id, screenshot_id, screenshot_kind, captured_at_ms, app_name, \
                window_title, summary \
         FROM ranked \
         WHERE rank_in_observation = 1 \
         ORDER BY captured_at_ms ASC",
    )
    .bind(start_ms)
    .bind(end_ms)
    .fetch_all(pool)
    .await
}

pub async fn count_screenshots_since(pool: &SqlitePool, since_ms: i64) -> Result<u32, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM activity_screenshots WHERE captured_at_ms >= ?")
            .bind(since_ms)
            .fetch_one(pool)
            .await?;
    Ok(count as u32)
}

pub async fn count_screenshots_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<u32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM activity_screenshots WHERE captured_at_ms >= ? AND captured_at_ms < ?",
    )
    .bind(start_ms)
    .bind(end_ms)
    .fetch_one(pool)
    .await?;
    Ok(count as u32)
}

pub async fn total_screenshot_storage_bytes(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let total: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(image_blob)), 0) FROM activity_screenshots")
            .fetch_one(pool)
            .await?;
    Ok(total as u64)
}
