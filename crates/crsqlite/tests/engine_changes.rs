//! Un-gated engine test: the changes surface (codec, Cursor paging,
//! pull/apply) against the **vendored** binary — no env var, no app schema.
//! The strict-schema fidelity gates stay in `tests/strict_roundtrip.rs`
//! (env-gated to a locally built extension when needed); this file pins the
//! engine contract on the exact bytes we ship.
//!
//! Exercises the tuple-paging loop the 0.7 transport will drive: pull a
//! page, apply it, advance the cursor, until `more == false`.

#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]

use sqlx::Row;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crsqlite::{Change, Cursor, SqlValue};

async fn open_node(tag: &str) -> (sqlx::sqlite::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite://{}",
        dir.path().join(format!("{tag}.db")).display()
    ))
    .unwrap()
    .create_if_missing(true);
    let (options, _) = crsqlite::apply(options).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE probe (id TEXT PRIMARY KEY NOT NULL, v TEXT, i INTEGER) STRICT")
        .execute(&pool)
        .await
        .unwrap();
    crsqlite::as_crr(&pool, "probe").await.unwrap();
    (pool, dir)
}

/// Pull every change after `cursor` through the paging loop (page by page
/// until `more == false`), returning them in order — the exact loop shape
/// the transport session drives.
async fn pull_all(pool: &sqlx::sqlite::SqlitePool, exclude_site: &[u8]) -> Vec<Change> {
    let mut out = Vec::new();
    let mut cursor = Cursor::START;
    loop {
        let page = crsqlite::pull_changes(pool, cursor, exclude_site, 4096)
            .await
            .unwrap();
        out.extend(page.changes);
        if !page.more {
            return out;
        }
        cursor = page.next;
    }
}

/// Small pages must tile the change stream exactly: every change appears
/// exactly once across pages, in `(db_version, seq)` order.
#[tokio::test]
async fn paged_pull_tiles_the_change_stream_exactly() {
    let (a, _dir_a) = open_node("page-a").await;

    // Enough writes to span several 4 KiB pages.
    for i in 0..64 {
        sqlx::query("INSERT INTO probe (id, v, i) VALUES (?, ?, ?)")
            .bind(format!("row-{i}"))
            .bind(format!("value-{i}"))
            .bind(i)
            .execute(&a)
            .await
            .unwrap();
    }

    let mut seen: Vec<(i64, i64)> = Vec::new();
    let mut cursor = Cursor::START;
    loop {
        let page = crsqlite::pull_changes(&a, cursor, &[0u8; 16], 4096)
            .await
            .unwrap();
        for change in &page.changes {
            seen.push((change.db_version, change.seq));
        }
        if !page.more {
            break;
        }
        cursor = page.next;
    }

    assert_eq!(seen.len(), 128, "every column change of every row appears");
    let mut sorted = seen.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len(), "no duplicate (db_version, seq)");
    assert!(
        seen.windows(2).all(|w| w[0] < w[1]),
        "pages must arrive in (db_version, seq) order, got {seen:?}"
    );

    a.close().await;
}

/// Two nodes: A writes the full storage-class matrix, B pulls through the
/// codec and applies; B must match A value-for-value and typeof-for-typeof,
/// re-apply must be a no-op, and a hard DELETE must tombstone.
#[tokio::test]
async fn two_node_roundtrip_over_the_changes_api() {
    let (a, _dir_a) = open_node("rt-a").await;
    let (b, _dir_b) = open_node("rt-b").await;

    let site_a = crsqlite::site_id(&a).await.unwrap();
    let site_b = crsqlite::site_id(&b).await.unwrap();
    assert_ne!(site_a, site_b, "fresh databases must get distinct sites");

    // The value matrix (probe table has no BLOB/ANY column — blobs come via
    // the pk/val codec itself; the STRICT fidelity probe covers those).
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('r1', '', 0)")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('r2', 'é€😀', -1)")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('r3', ?, ?)")
        .bind("p\0q")
        .bind(i64::MIN)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('r4', 'max', ?)")
        .bind(i64::MAX)
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('r5', NULL, NULL)")
        .execute(&a)
        .await
        .unwrap();

    // A → B. B excludes its own site (nothing of B's yet) — the wire shape.
    let changes = pull_all(&a, &site_b).await;
    assert!(!changes.is_empty());
    // The codec must see all five storage classes except blob (probe table
    // has none) — INTEGER, TEXT and NULL at least.
    let classes: std::collections::HashSet<&str> =
        changes.iter().map(|c| c.val.typeof_name()).collect();
    for expected in ["null", "integer", "text"] {
        assert!(
            classes.contains(expected),
            "missing {expected} in {classes:?}"
        );
    }

    // Apply in one caller-managed transaction (the transport's shape:
    // apply + cursor advance commit together).
    let mut tx = b.begin().await.unwrap();
    crsqlite::apply_changes(&mut *tx, &changes).await.unwrap();
    tx.commit().await.unwrap();

    // Value + typeof equality.
    let sql = "SELECT quote(v), typeof(v) FROM probe ORDER BY id";
    let rows_a = sqlx::query(sql).fetch_all(&a).await.unwrap();
    let rows_b = sqlx::query(sql).fetch_all(&b).await.unwrap();
    let fmt = |rows: Vec<sqlx::sqlite::SqliteRow>| -> Vec<String> {
        rows.iter()
            .map(|row| {
                let v: Option<String> = row.try_get(0).unwrap();
                let t: String = row.try_get(1).unwrap();
                format!("{v:?}|{t}")
            })
            .collect()
    };
    let fmt_a = fmt(rows_a);
    let fmt_b = fmt(rows_b);
    assert_eq!(fmt_a, fmt_b, "values + typeof must match across the wire");

    // Idempotency: re-applying the identical changeset is a no-op.
    let mut tx = b.begin().await.unwrap();
    crsqlite::apply_changes(&mut *tx, &changes).await.unwrap();
    tx.commit().await.unwrap();
    let rows_b2 = sqlx::query(sql).fetch_all(&b).await.unwrap();
    assert_eq!(fmt(rows_b2), fmt_a, "re-apply must not change anything");

    // Hard DELETE tombstones and propagates.
    sqlx::query("DELETE FROM probe WHERE id = 'r2'")
        .execute(&a)
        .await
        .unwrap();
    let after = Cursor {
        db_version: crsqlite::db_version(&a).await.unwrap() - 1,
        seq: i64::MAX,
    };
    let page = crsqlite::pull_changes(&a, after, &site_b, usize::MAX)
        .await
        .unwrap();
    assert!(
        page.changes.iter().any(|c| c.cid == "-1"),
        "hard DELETE must produce a tombstone (cid = -1)"
    );
    let mut tx = b.begin().await.unwrap();
    crsqlite::apply_changes(&mut *tx, &page.changes)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM probe WHERE id = 'r2'")
        .fetch_one(&b)
        .await
        .unwrap();
    assert_eq!(count, 0, "DELETE must propagate to B");

    // B → A direction.
    sqlx::query("INSERT INTO probe (id, v, i) VALUES ('from-b', 'hi', 1)")
        .execute(&b)
        .await
        .unwrap();
    let changes = pull_all(&b, &site_a).await;
    let mut tx = a.begin().await.unwrap();
    crsqlite::apply_changes(&mut *tx, &changes).await.unwrap();
    tx.commit().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM probe WHERE id = 'from-b'")
        .fetch_one(&a)
        .await
        .unwrap();
    assert_eq!(count, 1, "B→A direction must work");

    crsqlite::finalize(&a).await.unwrap();
    crsqlite::finalize(&b).await.unwrap();
    a.close().await;
    b.close().await;
}

/// Codec + wire round-trip of every storage class through a Change: the
/// serde round-trip must not lose the storage class (NULL vs 0 vs 0.0 vs
/// text vs blob).
#[test]
fn sql_value_serde_preserves_storage_classes() {
    for value in [
        SqlValue::Null,
        SqlValue::Integer(0),
        SqlValue::Integer(-1),
        SqlValue::Real(0.0),
        SqlValue::Text("".into()),
        SqlValue::Blob(vec![]),
        SqlValue::Blob(vec![0, 255, 1]),
    ] {
        let json = serde_json::to_string(&value).unwrap();
        let back: SqlValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value, "serde must round-trip {json}");
        assert_eq!(back.typeof_name(), value.typeof_name());
    }
}
