use sqlx::SqlitePool;

use crate::types::{ScreenshotAnalysisRow, SignalRow};

pub struct InsertSignal<'a> {
    pub id: &'a str,
    pub occurred_at_ms: i64,
    pub transition_sequence: i64,
    pub reason: &'a str,
    pub app_id: &'a str,
    pub bundle_id: &'a str,
    pub app_name: &'a str,
    pub activity_kind: &'a str,
    pub window_title: &'a str,
    pub url: &'a str,
    pub domain: &'a str,
    pub content_level: &'a str,
    pub source: &'a str,
    pub text_anchor_identity: &'a str,
    pub fingerprint: &'a str,
    pub payload_json: &'a str,
}

pub struct InsertScreenshot<'a> {
    pub id: &'a str,
    pub signal_id: &'a str,
    pub fingerprint: &'a str,
    pub captured_at_ms: i64,
    pub image_png: &'a [u8],
}

pub async fn insert_signal(pool: &SqlitePool, input: InsertSignal<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_signals ( \
            id, occurred_at_ms, transition_sequence, reason, app_id, bundle_id, app_name, activity_kind, \
            window_title, url, domain, content_level, source, text_anchor_identity, fingerprint, payload_json \
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.occurred_at_ms)
    .bind(input.transition_sequence)
    .bind(input.reason)
    .bind(input.app_id)
    .bind(input.bundle_id)
    .bind(input.app_name)
    .bind(input.activity_kind)
    .bind(input.window_title)
    .bind(input.url)
    .bind(input.domain)
    .bind(input.content_level)
    .bind(input.source)
    .bind(input.text_anchor_identity)
    .bind(input.fingerprint)
    .bind(input.payload_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_signals_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<SignalRow>, sqlx::Error> {
    sqlx::query_as::<_, SignalRow>(
        "SELECT id, occurred_at_ms, transition_sequence, reason, app_id, bundle_id, app_name, \
         activity_kind, window_title, url, domain, content_level, source, text_anchor_identity, \
         fingerprint, payload_json, created_at \
         FROM activity_signals \
         WHERE occurred_at_ms >= ? AND occurred_at_ms < ? \
         ORDER BY occurred_at_ms, transition_sequence, id",
    )
    .bind(start_ms)
    .bind(end_ms)
    .fetch_all(pool)
    .await
}

pub async fn list_screenshot_analyses_in_range(
    pool: &SqlitePool,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ScreenshotAnalysisRow>, sqlx::Error> {
    sqlx::query_as::<_, ScreenshotAnalysisRow>(
        "SELECT ss.fingerprint, sig.reason, ss.captured_at_ms, \
         sig.app_name, sig.window_title, ss.analysis_summary \
         FROM activity_screenshots ss \
         JOIN activity_signals sig ON sig.fingerprint = ss.fingerprint \
         WHERE ss.captured_at_ms >= ? AND ss.captured_at_ms < ? \
           AND ss.analysis_summary IS NOT NULL \
         GROUP BY ss.id \
         ORDER BY ss.captured_at_ms ASC",
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
        "INSERT INTO activity_screenshots (id, signal_id, fingerprint, captured_at_ms, image_png) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(input.id)
    .bind(input.signal_id)
    .bind(input.fingerprint)
    .bind(input.captured_at_ms)
    .bind(input.image_png)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_screenshot_analysis(
    pool: &SqlitePool,
    id: &str,
    summary: &str,
    analyzed_at_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE activity_screenshots SET analysis_summary = ?, analyzed_at_ms = ? WHERE id = ?",
    )
    .bind(summary)
    .bind(analyzed_at_ms)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
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
        sqlx::query_scalar("SELECT COALESCE(SUM(LENGTH(image_png)), 0) FROM activity_screenshots")
            .fetch_one(pool)
            .await?;
    Ok(total as u64)
}

pub async fn delete_oldest_screenshots_over_budget(
    pool: &SqlitePool,
    max_bytes: u64,
) -> Result<u32, sqlx::Error> {
    let total = total_screenshot_storage_bytes(pool).await?;
    if total <= max_bytes {
        return Ok(0);
    }

    let excess = (total - max_bytes) as i64;
    let result = sqlx::query(
        "DELETE FROM activity_screenshots \
         WHERE id IN ( \
           SELECT id FROM ( \
             SELECT id, \
                    COALESCE(SUM(LENGTH(image_png)) OVER ( \
                      ORDER BY captured_at_ms ASC \
                      ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING \
                    ), 0) AS freed_before \
             FROM activity_screenshots \
           ) \
           WHERE freed_before < ? \
         )",
    )
    .bind(excess)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as u32)
}

pub async fn insert_screenshot_and_cleanup(
    pool: &SqlitePool,
    input: InsertScreenshot<'_>,
    max_storage_bytes: u64,
) -> Result<(), sqlx::Error> {
    insert_screenshot(pool, input).await?;
    delete_oldest_screenshots_over_budget(pool, max_storage_bytes).await?;
    Ok(())
}
