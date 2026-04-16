use hypr_db_core::Db;
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::{ProxyQueryMethod, ProxyQueryResult};

pub async fn run_query(
    db: &Db,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = fetch_rows(db, sql, params).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

pub async fn run_query_proxy(
    db: &Db,
    sql: &str,
    params: &[serde_json::Value],
    method: ProxyQueryMethod,
) -> std::result::Result<ProxyQueryResult, sqlx::Error> {
    if method == ProxyQueryMethod::Run {
        bind_params(sqlx::query(sql), params)
            .execute(db.pool())
            .await?;
        return Ok(ProxyQueryResult { rows: Vec::new() });
    }

    let rows = fetch_rows(db, sql, params).await?;
    let rows = match method {
        ProxyQueryMethod::Get => rows
            .first()
            .map(row_to_json_array)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default(),
        ProxyQueryMethod::All | ProxyQueryMethod::Values => {
            rows.iter().map(row_to_json_array).collect()
        }
        ProxyQueryMethod::Run => Vec::new(),
    };

    Ok(ProxyQueryResult { rows })
}

async fn fetch_rows(
    db: &Db,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<sqlx::sqlite::SqliteRow>, sqlx::Error> {
    bind_params(sqlx::query(sql), params)
        .fetch_all(db.pool())
        .await
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = json_value_at(row, index);
        map.insert(column.name().to_string(), value);
    }

    serde_json::Value::Object(map)
}

fn row_to_json_array(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    serde_json::Value::Array(
        row.columns()
            .iter()
            .enumerate()
            .map(|(index, _)| json_value_at(row, index))
            .collect(),
    )
}

fn json_value_at(row: &sqlx::sqlite::SqliteRow, index: usize) -> serde_json::Value {
    match row.try_get_raw(index) {
        Ok(raw) if !raw.is_null() => match raw.type_info().name() {
            "BOOLEAN" => row
                .get::<Option<bool>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            "INTEGER" | "INT" => row
                .get::<Option<i64>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            "REAL" => row
                .get::<Option<f64>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            "BLOB" => row
                .get::<Option<Vec<u8>>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            _ => row
                .get::<Option<String>, _>(index)
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        },
        _ => serde_json::Value::Null,
    }
}

fn bind_params<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    params: &[serde_json::Value],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    query
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    async fn test_db() -> hypr_db_core::Db {
        hypr_db_core::Db::connect_memory_plain().await.unwrap()
    }

    #[tokio::test]
    async fn run_query_serializes_blob_null_and_boolean_values() {
        let db = test_db().await;

        sqlx::query(
            "CREATE TABLE query_types (
                id TEXT PRIMARY KEY NOT NULL,
                payload BLOB,
                enabled BOOLEAN,
                note TEXT
            )",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sqlx::query("INSERT INTO query_types (id, payload, enabled, note) VALUES (?, ?, ?, ?)")
            .bind("row-1")
            .bind(vec![0_u8, 1_u8, 2_u8, 255_u8])
            .bind(true)
            .bind(Option::<String>::None)
            .execute(db.pool())
            .await
            .unwrap();

        let rows = run_query(
            &db,
            "SELECT id, payload, enabled, note FROM query_types",
            &[],
        )
        .await
        .unwrap();

        assert_eq!(
            rows,
            vec![json!({
                "id": "row-1",
                "payload": [0, 1, 2, 255],
                "enabled": true,
                "note": serde_json::Value::Null,
            })]
        );
    }

    #[tokio::test]
    async fn run_query_binds_object_and_array_params_as_json_strings() {
        let db = test_db().await;
        let object_payload = json!({ "kind": "object", "count": 2 });
        let array_payload = json!(["a", "b"]);

        let rows = run_query(
            &db,
            "SELECT ? AS object_payload, ? AS array_payload, ? AS null_payload",
            &[
                object_payload.clone(),
                array_payload.clone(),
                serde_json::Value::Null,
            ],
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["null_payload"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(row["object_payload"].as_str().unwrap())
                .unwrap(),
            object_payload
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(row["array_payload"].as_str().unwrap())
                .unwrap(),
            array_payload
        );
    }

    #[tokio::test]
    async fn run_query_proxy_get_returns_empty_rows_when_query_is_empty() {
        let db = test_db().await;

        sqlx::query("CREATE TABLE proxy_values (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let result = run_query_proxy(
            &db,
            "SELECT id FROM proxy_values WHERE id = ?",
            &[json!("missing")],
            ProxyQueryMethod::Get,
        )
        .await
        .unwrap();

        assert!(result.rows.is_empty());
    }
}
