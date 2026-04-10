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

        assert_eq!(tables, vec!["activity_screenshots", "activity_signals"]);
    }

    #[tokio::test]
    async fn signal_and_screenshot_roundtrip() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_signal(
            db.pool(),
            InsertSignal {
                id: "sig1",
                occurred_at_ms: 1000,
                transition_sequence: 1,
                reason: "started",
                app_id: "com.apple.TextEdit",
                bundle_id: "com.apple.TextEdit",
                app_name: "TextEdit",
                activity_kind: "foreground_window",
                window_title: "Notes",
                url: "",
                domain: "",
                content_level: "full",
                source: "accessibility",
                text_anchor_identity: "",
                fingerprint: "fp1",
                payload_json: "{}",
            },
        )
        .await
        .unwrap();

        let signals = list_signals_in_range(db.pool(), 0, 2000).await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].fingerprint, "fp1");

        insert_screenshot(
            db.pool(),
            InsertScreenshot {
                id: "ss1",
                signal_id: "sig1",
                fingerprint: "fp1",
                captured_at_ms: 1100,
                image_png: &[1, 2, 3],
            },
        )
        .await
        .unwrap();

        update_screenshot_analysis(db.pool(), "ss1", "User is editing notes", 1200)
            .await
            .unwrap();

        let count = count_screenshots_since(db.pool(), 0).await.unwrap();
        assert_eq!(count, 1);

        let bytes = total_screenshot_storage_bytes(db.pool()).await.unwrap();
        assert_eq!(bytes, 3);
    }

    #[tokio::test]
    async fn screenshot_analyses_in_range() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        insert_signal(
            db.pool(),
            InsertSignal {
                id: "sig1",
                occurred_at_ms: 1000,
                transition_sequence: 1,
                reason: "app_changed",
                app_id: "com.apple.Safari",
                bundle_id: "com.apple.Safari",
                app_name: "Safari",
                activity_kind: "foreground_window",
                window_title: "Google",
                url: "",
                domain: "",
                content_level: "full",
                source: "accessibility",
                text_anchor_identity: "",
                fingerprint: "fp1",
                payload_json: "{}",
            },
        )
        .await
        .unwrap();

        insert_screenshot(
            db.pool(),
            InsertScreenshot {
                id: "ss1",
                signal_id: "sig1",
                fingerprint: "fp1",
                captured_at_ms: 1100,
                image_png: &[1, 2, 3],
            },
        )
        .await
        .unwrap();

        // No analysis yet — should return nothing
        let rows = list_screenshot_analyses_in_range(db.pool(), 0, 2000)
            .await
            .unwrap();
        assert!(rows.is_empty());

        update_screenshot_analysis(db.pool(), "ss1", "User browsing Google", 1200)
            .await
            .unwrap();

        let rows = list_screenshot_analyses_in_range(db.pool(), 0, 2000)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].fingerprint, "fp1");
        assert_eq!(rows[0].reason, "app_changed");
        assert_eq!(rows[0].app_name, "Safari");
        assert_eq!(rows[0].window_title, "Google");
        assert_eq!(rows[0].analysis_summary, "User browsing Google");
        assert_eq!(rows[0].captured_at_ms, 1100);

        // Out of range
        let rows = list_screenshot_analyses_in_range(db.pool(), 2000, 3000)
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn cleanup_oldest_screenshots() {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();

        for i in 0..5 {
            let sig_id = format!("sig{i}");
            let ss_id = format!("ss{i}");
            insert_signal(
                db.pool(),
                InsertSignal {
                    id: &sig_id,
                    occurred_at_ms: i * 1000,
                    transition_sequence: i,
                    reason: "started",
                    app_id: "",
                    bundle_id: "",
                    app_name: "App",
                    activity_kind: "foreground_window",
                    window_title: "",
                    url: "",
                    domain: "",
                    content_level: "metadata",
                    source: "accessibility",
                    text_anchor_identity: "",
                    fingerprint: &format!("fp{i}"),
                    payload_json: "{}",
                },
            )
            .await
            .unwrap();

            insert_screenshot(
                db.pool(),
                InsertScreenshot {
                    id: &ss_id,
                    signal_id: &sig_id,
                    fingerprint: &format!("fp{i}"),
                    captured_at_ms: i * 1000 + 100,
                    image_png: &vec![0u8; 100],
                },
            )
            .await
            .unwrap();
        }

        assert_eq!(count_screenshots_since(db.pool(), 0).await.unwrap(), 5);

        delete_oldest_screenshots_over_budget(db.pool(), 250)
            .await
            .unwrap();

        let remaining = count_screenshots_since(db.pool(), 0).await.unwrap();
        assert!(remaining <= 3, "expected <=3, got {remaining}");
        assert!(remaining >= 2, "expected >=2, got {remaining}");
    }
}
