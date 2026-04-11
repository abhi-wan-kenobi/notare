#![forbid(unsafe_code)]

mod daily_note_ops;
mod daily_note_types;
mod daily_summary_ops;
mod daily_summary_types;

pub use daily_note_ops::*;
pub use daily_note_types::*;
pub use daily_summary_ops::*;
pub use daily_summary_types::*;

use sqlx::SqlitePool;

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core2::Db3;

    async fn test_db() -> Db3 {
        let db = Db3::connect_memory_plain().await.unwrap();
        migrate(db.pool()).await.unwrap();
        db
    }

    #[tokio::test]
    async fn migrations_apply_cleanly() {
        let db = test_db().await;

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.0)
        .collect();

        assert_eq!(tables, vec!["daily_notes", "daily_summaries"]);
    }

    #[tokio::test]
    async fn daily_note_roundtrip() {
        let db = test_db().await;

        upsert_daily_note_body(db.pool(), "dn1", "2026-04-11", "{\"type\":\"doc\"}", "u1")
            .await
            .unwrap();

        let row = get_daily_note(db.pool(), "dn1").await.unwrap().unwrap();
        assert_eq!(row.date, "2026-04-11");
        assert_eq!(row.user_id, "u1");

        let by_date = get_daily_note_by_date(db.pool(), "2026-04-11", "u1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_date.id, "dn1");
    }

    #[tokio::test]
    async fn get_or_create_daily_note_is_idempotent() {
        let db = test_db().await;

        let first = get_or_create_daily_note(db.pool(), "dn1", "2026-04-11", "u1")
            .await
            .unwrap();
        let second = get_or_create_daily_note(db.pool(), "ignored", "2026-04-11", "u1")
            .await
            .unwrap();

        assert_eq!(first.id, "dn1");
        assert_eq!(second.id, "dn1");
    }

    #[tokio::test]
    async fn list_daily_notes_in_range_filters_by_user() {
        let db = test_db().await;

        upsert_daily_note_body(db.pool(), "dn1", "2026-04-10", "{}", "u1")
            .await
            .unwrap();
        upsert_daily_note_body(db.pool(), "dn2", "2026-04-11", "{}", "u1")
            .await
            .unwrap();
        upsert_daily_note_body(db.pool(), "dn3", "2026-04-11", "{}", "u2")
            .await
            .unwrap();

        let rows = list_daily_notes_in_range(db.pool(), "2026-04-10", "2026-04-11", "u1")
            .await
            .unwrap();
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();

        assert_eq!(ids, vec!["dn1", "dn2"]);
    }

    #[tokio::test]
    async fn daily_summary_roundtrip() {
        let db = test_db().await;

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds1",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "# Summary",
                timeline_json: "[{\"time\":\"09:00\"}]",
                topics_json: "[\"db\"]",
                status: "ready",
                source_cursor_ms: 123,
                source_fingerprint: "fp1",
                generation_error: "",
                generated_at: "2026-04-11T09:30:00Z",
            },
        )
        .await
        .unwrap();

        let row = get_daily_summary(db.pool(), "ds1").await.unwrap().unwrap();
        assert_eq!(row.status, "ready");
        assert_eq!(row.source_cursor_ms, 123);

        let by_date = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_date.id, "ds1");
    }

    #[tokio::test]
    async fn daily_summary_upsert_replaces_existing_row_for_daily_note() {
        let db = test_db().await;

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds1",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "first",
                timeline_json: "[]",
                topics_json: "[]",
                status: "ready",
                source_cursor_ms: 10,
                source_fingerprint: "a",
                generation_error: "",
                generated_at: "2026-04-11T09:00:00Z",
            },
        )
        .await
        .unwrap();

        upsert_daily_summary(
            db.pool(),
            UpsertDailySummary {
                id: "ds2",
                daily_note_id: "dn1",
                date: "2026-04-11",
                content: "second",
                timeline_json: "[1]",
                topics_json: "[2]",
                status: "ready",
                source_cursor_ms: 20,
                source_fingerprint: "b",
                generation_error: "",
                generated_at: "2026-04-11T10:00:00Z",
            },
        )
        .await
        .unwrap();

        let row = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.id, "ds1");
        assert_eq!(row.content, "second");
        assert_eq!(row.source_cursor_ms, 20);
    }

    #[tokio::test]
    async fn daily_summary_generating_and_error_helpers_work() {
        let db = test_db().await;

        mark_daily_summary_generating(db.pool(), "ds1", "dn1", "2026-04-11")
            .await
            .unwrap();
        let generating = get_daily_summary(db.pool(), "ds1").await.unwrap().unwrap();
        assert_eq!(generating.status, "generating");

        mark_daily_summary_error(db.pool(), "ds1", "dn1", "2026-04-11", "boom")
            .await
            .unwrap();
        let errored = get_daily_summary_by_date(db.pool(), "2026-04-11", "dn1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(errored.status, "error");
        assert_eq!(errored.generation_error, "boom");

        delete_daily_summary(db.pool(), "ds1").await.unwrap();
        assert!(get_daily_summary(db.pool(), "ds1").await.unwrap().is_none());
    }
}
