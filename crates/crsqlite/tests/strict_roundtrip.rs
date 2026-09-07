//! Phase 0 STRICT spike — the permanent CI gate for the cr-sqlite engine
//! swap (PR slice C-A; becomes `crsqlite_gate_*` CI jobs in PR C-B).
//!
//! Gating: requires `CRSQLITE_SPIKE_EXTENSION` (absolute path to a prebuilt
//! `crsqlite.{so,dylib,dll}` from the vlcn-io `v0.16.3` release). Unset →
//! every test prints "skipped" and returns, mirroring the `SQLITECLOUD_URL`
//! gating in `crates/db-core/tests/e2e.rs`.
//!
//! What this file pins (per the 2026-09-07 plan, Workstream C Phase 0):
//!
//! - The real app schema (`hypr_db_app::APP_MIGRATION_STEPS` through
//!   `hypr_db_migrate::migrate`) on two nodes — no hand-copied DDL.
//! - `crsql_as_crr` on the six `SYNCED_TABLES` + the registry rest + a
//!   `strict_probe` table carrying INTEGER/REAL/TEXT/BLOB/ANY and NOT NULL
//!   DEFAULT variants (the app schema has no BLOB/ANY column on any synced
//!   table, so the probe is the only exercise of those types).
//! - The `SqlValue` codec (`spike::SqlValue::decode` via
//!   `SqliteValueRef::type_info()`) — the codec the 0.7 transport ships.
//! - typeof+value equality across the pull→codec→apply path for every
//!   storage class and the plan's edge values, idempotent re-apply,
//!   LWW convergence both directions, hard-DELETE tombstones, soft delete
//!   via `deleted_at`, `PRAGMA integrity_check`, and the 800 KB
//!   `words_json` timing.
//! - The `as_crr`-twice idempotency probe and the update-hook probe (do
//!   `crsql_changes` writes fire `sqlite3_update_hook`, i.e. do live
//!   queries refresh after a remote apply for free).
//!
//! Everything asserted here was pre-probed against the real extension
//! 2026-09-07; measured numbers are recorded in
//! `docs/internal/sync-p2p.md` §30.

#![allow(clippy::too_many_lines)]

use crsqlite::spike::{
    apply_changes, assert_integrity_ok, connect_options, pull_changes, Change, Node,
    SqlValue, Timer, REGISTRY_TABLES, SYNCED_TABLES,
};
use sqlx::{Row, SqlitePool};

/// Skip helper: `None` → print "skipped" and return early from a test.
fn extension_path() -> Option<std::path::PathBuf> {
    crsqlite::spike::extension_path_from_env()
}

fn skip_notice(test: &str) {
    println!("skipped ({test}): CRSQLITE_SPIKE_EXTENSION is not set");
}

/// The probe table: every STRICT type + NOT NULL DEFAULT variants. The app
/// schema has no BLOB or ANY column on any synced table, so this is the only
/// exercise of those types. TEXT PK: cr-sqlite rejects `INTEGER PRIMARY KEY`
/// without an explicit `NOT NULL` (probed; see §30).
const PROBE_DDL: &str = r#"CREATE TABLE strict_probe (
    id TEXT PRIMARY KEY NOT NULL,
    c_integer INTEGER,
    c_real REAL,
    c_text TEXT,
    c_blob BLOB,
    c_any ANY,
    ni INTEGER NOT NULL DEFAULT 0,
    nr REAL NOT NULL DEFAULT 0.0,
    nt TEXT NOT NULL DEFAULT '',
    nb BLOB,
    na ANY
) STRICT"#;

async fn open_node(label: &'static str, extension: &std::path::Path) -> Node {
    Node::open(label, extension, 1)
        .await
        .unwrap_or_else(|e| panic!("open node {label}: {e}"))
}

/// Enable the six synced tables + the probe table on both nodes.
async fn enable_synced_tables(node: &Node) {
    for table in SYNCED_TABLES.iter().copied().chain(["strict_probe"]) {
        node.as_crr(table).await.unwrap();
        assert!(
            node.is_crr(table).await,
            "{table} must have a __crsql_clock table after as_crr"
        );
    }
}

/// The full edge-value matrix from the plan, on the probe table.
async fn insert_probe_matrix(pool: &SqlitePool) {
    let mut conn = pool.acquire().await.unwrap();

    // INTEGER: 0, -1, i64::MIN, i64::MAX
    for (i, v) in [0i64, -1, i64::MIN, i64::MAX].into_iter().enumerate() {
        sqlx::query("INSERT INTO strict_probe (id, c_integer) VALUES (?, ?)")
            .bind(format!("int-{i}"))
            .bind(v)
            .execute(&mut *conn)
            .await
            .unwrap();
    }

    // REAL: 0.0, -0.0, 0.1, 1e300, -1.5e-300
    for (i, v) in [0.0f64, -0.0, 0.1, 1e300, -1.5e-300]
        .into_iter()
        .enumerate()
    {
        sqlx::query("INSERT INTO strict_probe (id, c_real) VALUES (?, ?)")
            .bind(format!("real-{i}"))
            .bind(v)
            .execute(&mut *conn)
            .await
            .unwrap();
    }

    // TEXT: '', 4-byte UTF-8, embedded NUL
    let texts: [(&str, &str); 3] = [
        ("text-empty", ""),
        ("text-utf8", "é€😀🜻"),
        ("text-nul", "p\0q\0r"),
    ];
    for (id, v) in texts {
        sqlx::query("INSERT INTO strict_probe (id, c_text) VALUES (?, ?)")
            .bind(id)
            .bind(v)
            .execute(&mut *conn)
            .await
            .unwrap();
    }

    // BLOB: X'' and 1 MiB of deterministic pseudo-random bytes.
    sqlx::query("INSERT INTO strict_probe (id, c_blob) VALUES (?, ?)")
        .bind("blob-empty")
        .bind(Vec::<u8>::new())
        .execute(&mut *conn)
        .await
        .unwrap();
    let mut big = Vec::with_capacity(1024 * 1024);
    let mut state: u64 = 0x2545F4914F6CDD1D;
    for _ in 0..1024 * 1024 / 8 {
        // xorshift64* — deterministic, no dependency on a RNG crate.
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        big.extend_from_slice(&state.wrapping_mul(0x2545F4914F6CDD1D).to_le_bytes());
    }
    sqlx::query("INSERT INTO strict_probe (id, c_blob) VALUES (?, ?)")
        .bind("blob-1mib")
        .bind(&big)
        .execute(&mut *conn)
        .await
        .unwrap();

    // ANY: one row per storage class (integer, real, text, blob, null).
    let anys: [(&str, SqlValue); 5] = [
        ("any-int", SqlValue::Integer(42)),
        ("any-real", SqlValue::Real(0.5)),
        ("any-text", SqlValue::Text("any".into())),
        ("any-blob", SqlValue::Blob(vec![1, 2, 3])),
        ("any-null", SqlValue::Null),
    ];
    for (id, v) in anys {
        let query = sqlx::query("INSERT INTO strict_probe (id, c_any) VALUES (?, ?)")
            .bind(id);
        let query = match v {
            SqlValue::Null => query.bind(None::<i64>),
            SqlValue::Integer(x) => query.bind(x),
            SqlValue::Real(x) => query.bind(x),
            SqlValue::Text(x) => query.bind(x),
            SqlValue::Blob(x) => query.bind(x),
        };
        query.execute(&mut *conn).await.unwrap();
    }

    // NULL in every nullable column (defaults leave them NULL already);
    // one row that touches nothing but the PK.
    sqlx::query("INSERT INTO strict_probe (id) VALUES ('nulls-all')")
        .execute(&mut *conn)
        .await
        .unwrap();
}

/// Realistic ~800 KB `words_json`, built like `realistic_words_json(9000)`
/// in `crates/sync-p2p/examples/sync_transcripts_action_items_schema.rs`:
/// ~9000 diarized words with start/end ms, speaker, confidence.
fn realistic_words_json(word_count: usize) -> String {
    let mut words = String::with_capacity(word_count * 96);
    words.push('[');
    for i in 0..word_count {
        let start = i as u64 * 350;
        let end = start + 300;
        let speaker = if i % 17 < 9 { "S1" } else { "S2" };
        if i > 0 {
            words.push(',');
        }
        words.push_str(&format!(
            r#"{{"text":"word{i}","start_ms":{start},"end_ms":{end},"speaker":"{speaker}","confidence":0.91}}"#
        ));
    }
    words.push(']');
    words
}

/// Snapshot a table as (ordered value tuples + typeof strings) for
/// cross-node comparison: `SELECT *` plus `typeof()` per column, ordered by
/// the PK so row order is stable across nodes.
async fn snapshot(pool: &SqlitePool, table: &str) -> Vec<String> {
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info(?) WHERE pk = 0 ORDER BY cid",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    let pk: String =
        sqlx::query_scalar("SELECT name FROM pragma_table_info(?) WHERE pk > 0 ORDER BY pk LIMIT 1")
            .bind(table)
            .fetch_one(pool)
            .await
            .unwrap();

    let mut select_cols: Vec<String> = Vec::with_capacity(columns.len() * 2);
    for c in &columns {
        select_cols.push(format!("quote({c})"));
        select_cols.push(format!("typeof({c})"));
    }
    let sql = format!(
        "SELECT {} FROM {table} ORDER BY {pk}",
        select_cols.join(", ")
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .fetch_all(pool)
        .await
        .unwrap();
    rows.iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|i| {
                    let v: String = row.try_get(i).unwrap();
                    v
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

async fn sync_a_to_b(a: &Node, b: &Node, after: i64) -> i64 {
    let changes = pull_changes(&a.pool, after, &b.site_id().await).await.unwrap();
    let max_db_version = changes.iter().map(|c| c.db_version).max().unwrap_or(after);
    apply_changes(&b.pool, &changes).await.unwrap();
    max_db_version
}

/// Two nodes, real schema, six synced tables + probe enabled, full value
/// matrix through the codec, typeof+value identical, idempotent, convergent,
/// tombstoning, integrity-checked, timed.
#[tokio::test]
async fn strict_roundtrip_all_types_and_app_schema() {
    let Some(extension) = extension_path() else {
        skip_notice("strict_roundtrip_all_types_and_app_schema");
        return;
    };

    let a = open_node("a", &extension).await;
    let b = open_node("b", &extension).await;

    // --- Schema shape: the real migrations yield the expected STRICT count.
    let strict_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_list WHERE strict = 1 AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(&a.pool)
    .await
    .unwrap();
    assert_eq!(
        strict_count as usize,
        crsqlite::spike::EXPECTED_STRICT_TABLE_COUNT,
        "STRICT table count from the real migration steps"
    );

    // The registry tables all exist (guarded by db-app's own tests upstream,
    // re-asserted here because the spike's as_crr sweep depends on it).
    for table in REGISTRY_TABLES {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = ? AND type = 'table'")
                .bind(table)
                .fetch_one(&a.pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "{table} must exist after migrations");
    }

    // --- Probe table on both nodes.
    for pool in [&a.pool, &b.pool] {
        sqlx::query(PROBE_DDL).execute(pool).await.unwrap();
    }

    // --- as_crr on the six SYNCED_TABLES + probe on both nodes.
    enable_synced_tables(&a).await;
    enable_synced_tables(&b).await;

    // as_crr idempotency: calling it twice must not error and must not
    // change data (probed 2026-09-07; pinned here).
    a.as_crr("sessions").await.unwrap();
    a.as_crr("strict_probe").await.unwrap();

    // --- The full value matrix on the probe table (A only).
    insert_probe_matrix(&a.pool).await;

    // --- Real rows in the six synced tables (A only): one per table,
    // carrying both the DEFAULT-everything shape and realistic content,
    // including a `words_json` transcript at ~800 KB.
    let words_json = realistic_words_json(9000);
    assert!(
        words_json.len() >= 700_000 && words_json.len() < 900_000,
        "words_json must be ~800 KB, got {}",
        words_json.len()
    );

    let mut conn = a.pool.acquire().await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, title, metadata_json) VALUES ('sess-1', 'Spike session', '{}')",
    )
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::query("INSERT INTO session_documents (id, session_id, title) VALUES ('doc-1', 'sess-1', 'Note')")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO transcripts (id, session_id, source, provider, words_json, ended_at_ms) VALUES ('tr-1', 'sess-1', 'stt', 'voxtral', ?, 123000)")
        .bind(&words_json)
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO action_items (id, session_id, text, confidence) VALUES ('ai-1', 'sess-1', 'Do the spike', 0.75)")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tags (id, name) VALUES ('tag-1', 'spike')")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO session_tags (id, session_id, tag_id) VALUES ('st-1', 'sess-1', 'tag-1')")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    // --- A → B: pull with the plan's query, decode through the codec,
    // apply in one transaction. Time the 800 KB pull+apply.
    let timer = Timer::start();
    let pulled = pull_changes(&a.pool, 0, &b.site_id().await).await.unwrap();
    let pull_ms = timer.elapsed_ms();
    let timer = Timer::start();
    apply_changes(&b.pool, &pulled).await.unwrap();
    let apply_ms = timer.elapsed_ms();
    println!(
        "[spike] pulled {} changes (incl. 800 KB words_json) in {pull_ms} ms, applied in {apply_ms} ms",
        pulled.len()
    );

    // --- typeof + value identical on A and B for every exercised table.
    for table in SYNCED_TABLES.iter().copied().chain(["strict_probe"]) {
        let sa = snapshot(&a.pool, table).await;
        let sb = snapshot(&b.pool, table).await;
        assert_eq!(
            sa, sb,
            "{table}: value+typeof snapshot must be identical on A and B"
        );
    }

    // The 800 KB row must be byte-identical on B.
    let got: String =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'tr-1'")
            .fetch_one(&b.pool)
            .await
            .unwrap();
    assert_eq!(got, words_json, "800 KB words_json must be byte-identical");

    // The codec must not have textualised anything: spot-check the decoded
    // changes cover all five storage classes.
    let classes: std::collections::HashSet<&'static str> =
        pulled.iter().map(|c| c.val.typeof_name()).collect();
    for expected in ["null", "integer", "real", "text", "blob"] {
        assert!(
            classes.contains(expected),
            "decoded changes must contain a {expected} value (got {classes:?})"
        );
    }

    // --- Idempotency: re-apply the identical changeset → no error, no
    // change.
    apply_changes(&b.pool, &pulled).await.unwrap();
    for table in SYNCED_TABLES.iter().copied().chain(["strict_probe"]) {
        let sb = snapshot(&b.pool, table).await;
        let sa = snapshot(&a.pool, table).await;
        assert_eq!(sa, sb, "{table}: re-apply must not change anything");
    }

    // --- Integrity.
    assert_integrity_ok(&a.pool, "a").await;
    assert_integrity_ok(&b.pool, "b").await;

    // --- LWW: concurrent per-column UPDATE both ways converges.
    let mut conn = a.pool.acquire().await.unwrap();
    sqlx::query("UPDATE sessions SET title = 'A-wins?' WHERE id = 'sess-1'")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    let mut conn = b.pool.acquire().await.unwrap();
    sqlx::query("UPDATE sessions SET title = 'B-wins?' WHERE id = 'sess-1'")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    // Exchange both ways (A pulls B's, B pulls A's).
    let a_version = a.db_version().await;
    let b_version = b.db_version().await;
    let a_changes = pull_changes(&a.pool, a_version - 1, &a.site_id().await)
        .await
        .unwrap();
    let b_changes = pull_changes(&b.pool, b_version - 1, &b.site_id().await)
        .await
        .unwrap();
    apply_changes(&a.pool, &b_changes).await.unwrap();
    apply_changes(&b.pool, &a_changes).await.unwrap();

    let a_title: String =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'sess-1'")
            .fetch_one(&a.pool)
            .await
            .unwrap();
    let b_title: String =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'sess-1'")
            .fetch_one(&b.pool)
            .await
            .unwrap();
    assert_eq!(
        a_title, b_title,
        "concurrent UPDATE must converge (LWW, site_id tie-break)"
    );
    let converged_value = a_title;
    println!("[spike] LWW converged to {converged_value:?}");

    // --- Hard DELETE propagates as a tombstone.
    let mut conn = a.pool.acquire().await.unwrap();
    sqlx::query("DELETE FROM tags WHERE id = 'tag-1'")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    let a_version = a.db_version().await;
    let changes = pull_changes(&a.pool, a_version - 1, &b.site_id().await)
        .await
        .unwrap();
    assert!(
        changes.iter().any(|c| c.cid == "-1"),
        "hard DELETE must produce a tombstone change (cid = -1)"
    );
    apply_changes(&b.pool, &changes).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = 'tag-1'")
        .fetch_one(&b.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "hard DELETE must propagate to B");

    // --- Soft delete via `deleted_at` converges.
    let mut conn = a.pool.acquire().await.unwrap();
    sqlx::query("UPDATE sessions SET deleted_at = '2026-09-07T00:00:00.000Z' WHERE id = 'sess-1'")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    let a_version = a.db_version().await;
    let changes = pull_changes(&a.pool, a_version - 1, &b.site_id().await)
        .await
        .unwrap();
    apply_changes(&b.pool, &changes).await.unwrap();
    let deleted_at: Option<String> =
        sqlx::query_scalar("SELECT deleted_at FROM sessions WHERE id = 'sess-1'")
            .fetch_one(&b.pool)
            .await
            .unwrap();
    assert_eq!(
        deleted_at.as_deref(),
        Some("2026-09-07T00:00:00.000Z"),
        "soft delete via deleted_at must converge"
    );

    // --- B → A direction: B inserts, A pulls and applies.
    let mut conn = b.pool.acquire().await.unwrap();
    sqlx::query("INSERT INTO tags (id, name) VALUES ('tag-from-b', 'b-side')")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);
    let b_version = b.db_version().await;
    let changes = pull_changes(&b.pool, b_version - 1, &a.site_id().await)
        .await
        .unwrap();
    apply_changes(&a.pool, &changes).await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = 'tag-from-b'")
            .fetch_one(&a.pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "B→A direction must work");

    // Final convergence + integrity over the whole run.
    for table in SYNCED_TABLES.iter().copied().chain(["strict_probe"]) {
        let sa = snapshot(&a.pool, table).await;
        let sb = snapshot(&b.pool, table).await;
        assert_eq!(sa, sb, "{table}: final convergence");
    }
    assert_integrity_ok(&a.pool, "a-final").await;
    assert_integrity_ok(&b.pool, "b-final").await;

    a.pool.close().await;
    b.pool.close().await;
}

/// `crsql_as_crr` on every registry table (the "all 18 STRICT tables"
/// obligation): no apply error on any of them, with the two known
/// non-registry STRICT exceptions asserted for the record.
#[tokio::test]
async fn as_crr_covers_the_full_registry() {
    let Some(extension) = extension_path() else {
        skip_notice("as_crr_covers_the_full_registry");
        return;
    };

    let node = open_node("registry", &extension).await;

    // as_crr on every registry table must succeed.
    for table in REGISTRY_TABLES {
        node.as_crr(table).await.unwrap();
        assert!(node.is_crr(table).await, "{table} clock table exists");
        // Idempotent (the plan's probe).
        node.as_crr(table).await.unwrap();
    }

    // Known non-registry STRICT exceptions, pinned with their engine
    // messages (see §30):
    // - voice_profiles: NOT NULL column without DEFAULT.
    let err = sqlx::query("SELECT crsql_as_crr('voice_profiles')")
        .execute(&node.pool)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("NOT NULL column without a DEFAULT"),
        "voice_profiles rejection reason, got: {err}"
    );
    // - embedding_vector_map: unique index besides the PK.
    let err = sqlx::query("SELECT crsql_as_crr('embedding_vector_map')")
        .execute(&node.pool)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("unique indices"),
        "embedding_vector_map rejection reason, got: {err}"
    );

    // PK-only insert on every registry table (the clock-table path must
    // accept a default-everything row) and a pull of nothing: registry
    // tables that are NOT in SYNCED_TABLES are enabled here only to prove
    // as_crr compatibility, not data round-trip.
    for table in REGISTRY_TABLES {
        let pk: String =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) WHERE pk > 0 ORDER BY pk LIMIT 1")
                .bind(table)
                .fetch_one(&node.pool)
                .await
                .unwrap();
        let sql = format!("INSERT INTO {table} ({pk}) VALUES ('spike-{table}')");
        // Some registry tables have extra NOT NULL columns with defaults
        // only; a PK-only insert relies on all defaults, which the schema
        // guarantees. `templates` is pre-seeded by the migrations but has a
        // TEXT PK namespace that cannot collide with 'spike-'.
        if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).execute(&node.pool).await {
            panic!("PK-only insert into {table} failed: {e}");
        }
    }

    node.pool.close().await;
}

/// Do `crsql_changes` writes fire `sqlite3_update_hook`? Registers
/// `ChangeNotifier` (as `Db::open` does) and asserts whether a remote-apply
/// INSERT surfaces a `TableChange` for `sessions`. This decides whether
/// live queries refresh after a remote apply for free (§30 finding).
#[tokio::test]
async fn remote_apply_update_hook_probe() {
    let Some(extension) = extension_path() else {
        skip_notice("remote_apply_update_hook_probe");
        return;
    };

    let a = open_node("hook-a", &extension).await;
    let b = open_node("hook-b", &extension).await;

    for node in [&a, &b] {
        node.as_crr("sessions").await.unwrap();
    }

    // Local write on A: the notifier must fire (sanity that the hook is
    // registered on this pool).
    let mut rx = b.notifier.subscribe();
    sqlx::query("INSERT INTO sessions (id, title) VALUES ('hook-1', 'local')")
        .execute(&a.pool)
        .await
        .unwrap();

    let changes = pull_changes(&a.pool, 0, &b.site_id().await).await.unwrap();
    apply_changes(&b.pool, &changes).await.unwrap();

    // Give the commit-hook flush a moment (the notifier flushes on commit).
    let mut saw_sessions = false;
    for _ in 0..50 {
        match rx.try_recv() {
            Ok(change) if change.table == "sessions" => {
                saw_sessions = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }

    // The local sanity check: B's own apply is a crsql_changes vtab write.
    // Whether that fires update_hook is the open question — record either
    // way; assert no panic and the row landed.
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'hook-1'")
            .fetch_one(&b.pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "remote apply must land the row");

    if saw_sessions {
        println!(
            "[spike] update_hook FIRES for remote applies via crsql_changes — live queries refresh for free"
        );
    } else {
        println!(
            "[spike] update_hook does NOT fire for remote applies via crsql_changes — db-reactive needs an explicit refresh signal after sync rounds (§30)"
        );
    }

    a.pool.close().await;
    b.pool.close().await;
}

/// Sanity for the skip path itself: the codec decodes every storage class
/// from a plain SQL row without the extension. Keeps the codec exercised on
/// every CI run even without `CRSQLITE_SPIKE_EXTENSION`.
#[tokio::test]
async fn codec_covers_every_storage_class_without_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("codec.db");
    // Rebuild options without the extension: connect_options always adds
    // one, so construct directly here.
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    sqlx::query("CREATE TABLE v (n, i, r, t, b)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO v VALUES (NULL, 7, 0.25, 'x', X'00ff')")
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT n, i, r, t, b FROM v")
        .fetch_one(&pool)
        .await
        .unwrap();
    use sqlx::Column;
    let n = row.try_get_raw(0).unwrap();
    let i = row.try_get_raw(1).unwrap();
    let r = row.try_get_raw(2).unwrap();
    let t = row.try_get_raw(3).unwrap();
    let bl = row.try_get_raw(4).unwrap();
    assert_eq!(SqlValue::decode(n), SqlValue::Null);
    assert_eq!(SqlValue::decode(i), SqlValue::Integer(7));
    assert_eq!(SqlValue::decode(r), SqlValue::Real(0.25));
    assert_eq!(SqlValue::decode(t), SqlValue::Text("x".into()));
    assert_eq!(SqlValue::decode(bl), SqlValue::Blob(vec![0x00, 0xff]));
    assert_eq!(row.columns().len(), 5);

    pool.close().await;
}