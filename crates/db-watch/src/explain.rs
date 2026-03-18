use std::collections::{HashMap, HashSet};

use sqlx::{Row, SqlitePool};

/// Run `EXPLAIN <sql>` and return the set of table names the query reads.
///
/// Works with joins, subqueries, CTEs — anything SQLite can plan.
/// Bound parameters (`?`) are irrelevant to EXPLAIN (it analyzes the plan, not data).
///
/// # Safety
///
/// `sql` is interpolated into `format!("EXPLAIN {sql}")` and executed directly.
/// Only pass SQL from trusted code (e.g. compile-time query strings), never user input.
pub async fn extract_tables(pool: &SqlitePool, sql: &str) -> Result<HashSet<String>, sqlx::Error> {
    // Build rootpage -> table_name mapping from sqlite_master.
    // Both tables and indexes map to tbl_name, so index lookups resolve correctly.
    let master_rows =
        sqlx::query("SELECT rootpage, tbl_name FROM sqlite_master WHERE rootpage > 0")
            .fetch_all(pool)
            .await?;

    let mut page_to_table: HashMap<i32, String> = HashMap::new();
    for row in &master_rows {
        let rootpage: i32 = row.get("rootpage");
        let tbl_name: String = row.get("tbl_name");
        page_to_table.insert(rootpage, tbl_name);
    }

    // Run EXPLAIN and collect tables from OpenRead opcodes via p2 (root page).
    let explain_rows = sqlx::query(&format!("EXPLAIN {sql}"))
        .fetch_all(pool)
        .await?;

    let mut tables = HashSet::new();
    for row in &explain_rows {
        let opcode: &str = row.get("opcode");
        if opcode != "OpenRead" {
            continue;
        }
        let p2: i32 = row.get("p2");
        if let Some(tbl) = page_to_table.get(&p2) {
            if !tbl.starts_with("sqlite_") {
                tables.insert(tbl.clone());
            }
        }
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db.pool().clone()
    }

    #[tokio::test]
    async fn single_table() {
        let pool = test_pool().await;
        let tables = extract_tables(&pool, "SELECT id FROM sessions WHERE id = ?")
            .await
            .unwrap();
        assert_eq!(tables, HashSet::from(["sessions".to_string()]));
    }

    #[tokio::test]
    async fn join_query() {
        let pool = test_pool().await;
        let tables = extract_tables(
            &pool,
            "SELECT w.id FROM words w JOIN sessions s ON w.session_id = s.id",
        )
        .await
        .unwrap();
        assert!(tables.contains("words"));
        assert!(tables.contains("sessions"));
        assert_eq!(tables.len(), 2);
    }

    #[tokio::test]
    async fn subquery() {
        let pool = test_pool().await;
        let tables = extract_tables(
            &pool,
            "SELECT id FROM sessions WHERE id IN (SELECT session_id FROM words)",
        )
        .await
        .unwrap();
        assert!(tables.contains("sessions"));
        assert!(tables.contains("words"));
        assert_eq!(tables.len(), 2);
    }
}
