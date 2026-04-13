#![forbid(unsafe_code)]

mod ops;
mod types;

pub use ops::*;
pub use types::*;

use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    #[tokio::test]
    async fn migrations_apply_cleanly() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect();

        assert_eq!(
            tables,
            vec![
                "activity_observation_analyses",
                "activity_observation_events",
                "activity_screenshots",
            ]
        );
    }

    #[tokio::test]
    async fn observation_event_and_analysis_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_observation_event(
            db.pool(),
            InsertObservationEvent {
                id: "obs_evt_1",
                observation_id: "obs_1",
                occurred_at_ms: 1_000,
                event_kind: "started",
                end_reason: None,
                change_class: Some("structural"),
                app_id: "com.apple.Mail",
                bundle_id: "com.apple.Mail",
                app_name: "Mail",
                activity_kind: "foreground_window",
                window_title: "Compose",
                url: "",
                domain: "",
                text_anchor_identity: "compose:body",
                observation_key: "mail|foreground_window|window:10|compose:body",
                snapshot_json: "{}",
            },
        )
        .await
        .unwrap();

        insert_screenshot(
            db.pool(),
            InsertScreenshot {
                id: "ss_1",
                observation_id: "obs_1",
                screenshot_kind: "settled",
                scheduled_at_ms: 900,
                captured_at_ms: 1_100,
                app_name: "Mail",
                window_title: "Compose",
                mime_type: "image/png",
                width: 100,
                height: 50,
                sha256: "hash",
                image_blob: &[1, 2, 3],
                snapshot_json: "{}",
            },
        )
        .await
        .unwrap();

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_1",
                observation_id: "obs_1",
                screenshot_id: "ss_1",
                screenshot_kind: "settled",
                captured_at_ms: 1_100,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Mail",
                window_title: "Compose",
                summary: "User is drafting an email reply.",
            },
        )
        .await
        .unwrap();

        let events = list_observation_events_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].observation_id, "obs_1");

        let analyses = list_preferred_observation_analyses_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].summary, "User is drafting an email reply.");
        assert_eq!(analyses[0].screenshot_kind, "settled");

        assert_eq!(count_screenshots_since(db.pool(), 0).await.unwrap(), 1);
        assert_eq!(total_screenshot_storage_bytes(db.pool()).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn preferred_analysis_prefers_settled_over_entry() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_entry",
                observation_id: "obs_1",
                screenshot_id: "ss_entry",
                screenshot_kind: "entry",
                captured_at_ms: 1_000,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Safari",
                window_title: "Example",
                summary: "Entry view",
            },
        )
        .await
        .unwrap();

        insert_observation_analysis(
            db.pool(),
            InsertObservationAnalysis {
                id: "oa_settled",
                observation_id: "obs_1",
                screenshot_id: "ss_settled",
                screenshot_kind: "settled",
                captured_at_ms: 1_100,
                model_name: "local-vlm",
                prompt_version: "v1",
                app_name: "Safari",
                window_title: "Example",
                summary: "Settled view",
            },
        )
        .await
        .unwrap();

        let analyses = list_preferred_observation_analyses_in_range(db.pool(), 0, 2_000)
            .await
            .unwrap();
        assert_eq!(analyses.len(), 1);
        assert_eq!(analyses[0].summary, "Settled view");
        assert_eq!(analyses[0].screenshot_kind, "settled");
    }
}
