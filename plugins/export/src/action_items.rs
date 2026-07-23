//! Action-item export (WS-D2). Queries the `action_items` rows for one session
//! (or all sessions) and serializes them to CSV/JSON via `hypr-export-core`.
//! SQLite is authoritative — this is a read-only projection.

use hypr_export_core::ActionItemExport;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

/// Output format for [`export_action_items`](crate::commands::export_action_items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ExportActionItemsFormat {
    Csv,
    Json,
}

/// The exact columns we project out of `action_items`. `owner` is the raw
/// `owner_speaker_id` (there is no cross-session speaker→human label table to
/// resolve against at export time).
#[derive(Debug, Clone, sqlx::FromRow)]
struct ActionItemRow {
    text: String,
    owner_speaker_id: String,
    due_at: String,
    status: String,
    priority: String,
    confidence: f64,
    source_text: String,
}

impl From<ActionItemRow> for ActionItemExport {
    fn from(row: ActionItemRow) -> Self {
        ActionItemExport {
            text: row.text,
            owner: row.owner_speaker_id,
            due_at: row.due_at,
            status: row.status,
            priority: row.priority,
            confidence: row.confidence,
            source_text: row.source_text,
        }
    }
}

/// Load the action items to export. `session_id = None` exports every session.
pub async fn collect_action_items(
    pool: &SqlitePool,
    session_id: Option<&str>,
) -> Result<Vec<ActionItemExport>, sqlx::Error> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT text, owner_speaker_id, due_at, status, priority, confidence, source_text
         FROM action_items
         WHERE deleted_at IS NULL",
    );
    if let Some(session_id) = session_id {
        query.push(" AND session_id = ");
        query.push_bind(session_id);
    }
    query.push(" ORDER BY session_id, source_order, created_at, id");

    let rows = query
        .build_query_as::<ActionItemRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(ActionItemExport::from).collect())
}

/// Serialize items into a file payload in the requested format.
pub fn serialize(
    items: &[ActionItemExport],
    format: ExportActionItemsFormat,
) -> crate::Result<String> {
    match format {
        ExportActionItemsFormat::Csv => Ok(hypr_export_core::to_csv(items)),
        ExportActionItemsFormat::Json => hypr_export_core::to_json(items),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed() -> SqlitePool {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        let pool = db.pool().clone();

        for (id, session, order, status, text, owner, due, prio, conf, src) in [
            (
                "ai-1",
                "s1",
                0,
                "todo",
                "Send budget",
                "spk_1",
                "2026-07-24",
                "high",
                0.9,
                "we should send the budget",
            ),
            (
                "ai-2",
                "s1",
                1,
                "done",
                "Book venue",
                "",
                "",
                "low",
                0.5,
                "book the venue",
            ),
            (
                "ai-3",
                "s2",
                0,
                "todo",
                "Other session task",
                "spk_2",
                "",
                "",
                0.1,
                "do other",
            ),
        ] {
            sqlx::query(
                "INSERT INTO action_items
                 (id, session_id, source_order, status, text, owner_speaker_id, due_at, priority, confidence, source_text)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(session)
            .bind(order)
            .bind(status)
            .bind(text)
            .bind(owner)
            .bind(due)
            .bind(prio)
            .bind(conf)
            .bind(src)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn collects_only_the_requested_session_in_order() {
        let pool = seed().await;
        let items = collect_action_items(&pool, Some("s1")).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "Send budget");
        assert_eq!(items[0].owner, "spk_1");
        assert_eq!(items[1].text, "Book venue");
        assert_eq!(items[1].status, "done");
    }

    #[tokio::test]
    async fn collects_all_sessions_when_unfiltered() {
        let pool = seed().await;
        let items = collect_action_items(&pool, None).await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn serializes_csv_and_json() {
        let pool = seed().await;
        let items = collect_action_items(&pool, Some("s1")).await.unwrap();

        let csv = serialize(&items, ExportActionItemsFormat::Csv).unwrap();
        assert!(csv.starts_with("text,owner,due_at,status,priority,confidence,source_text\r\n"));
        assert!(
            csv.contains("Send budget,spk_1,2026-07-24,todo,high,0.9,we should send the budget")
        );

        let json = serialize(&items, ExportActionItemsFormat::Json).unwrap();
        assert!(json.contains("\"text\": \"Send budget\""));
        assert!(json.contains("\"status\": \"done\""));
    }
}
