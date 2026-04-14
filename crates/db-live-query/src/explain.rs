use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;

use crate::DependencyTarget;
use crate::schema::{CatalogStore, DependencyResolutionError};

/// # Safety
///
/// `sql` is interpolated into `format!("EXPLAIN QUERY PLAN {sql}")` and executed directly.
/// Only pass SQL from trusted code, never user input.
pub async fn extract_dependencies(
    pool: &SqlitePool,
    sql: &str,
) -> Result<HashSet<DependencyTarget>, DependencyResolutionError> {
    CatalogStore::default()
        .analyze_query(pool, sql)
        .await
        .map(|resolved| resolved.targets)
}

pub(crate) fn parse_table_from_detail(detail: &str) -> Option<&str> {
    let trimmed = detail.trim();
    let rest = trimmed
        .strip_prefix("SCAN ")
        .or_else(|| trimmed.strip_prefix("SEARCH "))?;
    rest.split_whitespace().next()
}

pub(crate) fn normalize_identifier(token: &str) -> String {
    let token = token.trim_matches(|c: char| matches!(c, ',' | ')' | ';' | '('));
    let token = token.rsplit('.').next().unwrap_or(token);
    strip_identifier_quotes(token).to_string()
}

fn strip_identifier_quotes(token: &str) -> &str {
    if token.len() >= 2 {
        if (token.starts_with('"') && token.ends_with('"'))
            || (token.starts_with('`') && token.ends_with('`'))
            || (token.starts_with('[') && token.ends_with(']'))
        {
            return &token[1..token.len() - 1];
        }
    }

    token
}

pub(crate) fn build_alias_map(
    sql: &str,
    known_objects: &HashSet<String>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let upper = sql.to_uppercase();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let upper_tokens: Vec<&str> = upper.split_whitespace().collect();

    for i in 0..tokens.len() {
        let is_from_or_join = matches!(
            upper_tokens[i],
            "FROM" | "JOIN" | "INNER" | "LEFT" | "RIGHT" | "CROSS"
        );
        if !is_from_or_join {
            continue;
        }

        let table_idx = if matches!(upper_tokens[i], "INNER" | "LEFT" | "RIGHT" | "CROSS") {
            if i + 1 < tokens.len() && upper_tokens[i + 1] == "JOIN" {
                i + 2
            } else {
                continue;
            }
        } else {
            i + 1
        };

        if table_idx >= tokens.len() {
            continue;
        }

        let raw_object = normalize_identifier(tokens[table_idx]);
        if !known_objects.contains(&raw_object) {
            continue;
        }

        let alias_idx = if table_idx + 1 < upper_tokens.len() && upper_tokens[table_idx + 1] == "AS"
        {
            table_idx + 2
        } else {
            table_idx + 1
        };

        if alias_idx < tokens.len() {
            let alias = normalize_identifier(tokens[alias_idx]);
            let alias_upper = alias.to_uppercase();
            if !alias.is_empty()
                && !matches!(
                    alias_upper.as_str(),
                    "ON" | "WHERE"
                        | "SET"
                        | "JOIN"
                        | "INNER"
                        | "LEFT"
                        | "RIGHT"
                        | "CROSS"
                        | "ORDER"
                        | "GROUP"
                        | "HAVING"
                        | "LIMIT"
                        | "UNION"
                        | "EXCEPT"
                        | "INTERSECT"
                )
                && !known_objects.contains(&alias)
            {
                map.insert(alias, raw_object.clone());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> hypr_db_core2::Db3 {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db
    }

    #[tokio::test]
    async fn single_table() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT id FROM daily_notes WHERE id = ?",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn join_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id",
        )
        .await
        .unwrap();
        assert!(targets.contains(&DependencyTarget::Table("daily_summaries".to_string())));
        assert!(targets.contains(&DependencyTarget::Table("daily_notes".to_string())));
        assert_eq!(targets.len(), 2);
    }

    #[tokio::test]
    async fn alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT dn.id FROM daily_notes AS dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn subquery() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT id FROM daily_notes \
             WHERE EXISTS ( \
               SELECT 1 FROM daily_summaries \
               WHERE daily_summaries.daily_note_id = daily_notes.id \
             )",
        )
        .await
        .unwrap();
        assert!(targets.contains(&DependencyTarget::Table("daily_notes".to_string())));
        assert!(targets.contains(&DependencyTarget::Table("daily_summaries".to_string())));
        assert_eq!(targets.len(), 2);
    }

    #[tokio::test]
    async fn quoted_alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            r#"SELECT "dn".id FROM "daily_notes" AS "dn" WHERE "dn".date = '2026-04-11'"#,
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn bracket_quoted_alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT [dn].id FROM [daily_notes] AS [dn] WHERE [dn].date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn schema_qualified_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT dn.id FROM main.daily_notes dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn view_query_resolves_to_base_table() {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();
        sqlx::query("CREATE VIEW notes_view AS SELECT id, body FROM notes")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT id FROM notes_view WHERE body IS NOT NULL",
        )
        .await
        .unwrap();

        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("notes".to_string())])
        );
    }

    #[tokio::test]
    async fn fts_match_query_resolves_to_virtual_table_target() {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let targets = extract_dependencies(
            db.pool().as_ref(),
            "SELECT rowid, title FROM docs_fts WHERE docs_fts MATCH 'hello'",
        )
        .await
        .unwrap();

        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::VirtualTable("docs_fts".to_string())])
        );
    }

    #[tokio::test]
    async fn empty_dependency_set_is_non_reactive() {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        let error = extract_dependencies(db.pool().as_ref(), "SELECT 1")
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DependencyResolutionError::EmptyDependencySet
        ));
    }

    #[tokio::test]
    async fn unsupported_virtual_tables_are_non_reactive() {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_rtree USING rtree(id, min_x, max_x)")
            .execute(db.pool().as_ref())
            .await
            .unwrap();

        let error = extract_dependencies(db.pool().as_ref(), "SELECT id FROM docs_rtree")
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DependencyResolutionError::UnsupportedObject { .. }
        ));
    }
}
