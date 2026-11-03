//! `CrsqliteChangeSource` against the real app schema on two tempfile
//! databases, no transport: what one device serves, the other applies.

#![cfg(all(feature = "sync", sync_platform))]

use std::sync::Arc;

use hypr_db_core::Db;
use sqlx::Row;
use sync_p2p::{ChangeSource, WireCursor, WirePage};
use tauri_plugin_db::change_source::{CrsqliteChangeSource, enable_crr_tables};

struct Node {
    db: Arc<Db>,
    source: CrsqliteChangeSource,
    path: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

impl Node {
    async fn open() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        let db = Arc::new(open_db(&path).await);
        enable_crr_tables(&db).await.unwrap();
        let source = CrsqliteChangeSource::new(Arc::clone(&db));
        Self {
            db,
            source,
            path,
            _dir: dir,
        }
    }

    async fn site(&self) -> Vec<u8> {
        self.source.site_id().await.unwrap()
    }

    async fn exec(&self, sql: &str) {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(self.db.pool())
            .await
            .unwrap();
    }

    /// Every clock entry, in stream order: what a re-apply must not touch.
    async fn clock_state(&self) -> Vec<(String, Vec<u8>, String, i64, i64, i64, Vec<u8>)> {
        sqlx::query_as(
            r#"SELECT "table", pk, cid, col_version, db_version, seq, site_id
               FROM crsql_changes ORDER BY db_version, seq"#,
        )
        .fetch_all(self.db.pool())
        .await
        .unwrap()
    }

    /// Every change after `after` that `requester` is entitled to, drained
    /// page by page with a small page size.
    async fn serve_all(&self, after: WireCursor, requester: &Node) -> WirePage {
        let exclude = Some(requester.site().await);
        let mut all = Vec::new();
        let mut cursor = after;
        loop {
            let page = self
                .source
                .changes_since(cursor, exclude.clone(), 3)
                .await
                .unwrap();
            all.extend(page.changes);
            cursor = page.next;
            if !page.more {
                return WirePage {
                    changes: all,
                    next: cursor,
                    more: false,
                };
            }
        }
    }

    /// `from`'s changes since the start of its stream, applied here.
    async fn pull_from(&self, from: &Node) {
        let page = from.serve_all(WireCursor::default(), self).await;
        self.source.apply(page.changes).await.unwrap();
    }

    /// Full contents of `table`, every column, by storage class.
    async fn snapshot(&self, table: &str) -> Vec<Vec<hypr_crsqlite::SqlValue>> {
        let sql = format!("SELECT * FROM {table} ORDER BY id");
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(self.db.pool())
            .await
            .unwrap();
        rows.iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| hypr_crsqlite::SqlValue::decode(row.try_get_raw(i).unwrap()).unwrap())
                    .collect()
            })
            .collect()
    }

    async fn title(&self, id: &str) -> String {
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_one(self.db.pool())
            .await
            .unwrap()
    }

    async fn close(self) {
        self.db.close().await;
    }
}

async fn open_db(path: &std::path::Path) -> Db {
    tauri_plugin_db::open_app_db_with_crsqlite(Some(path), true)
        .await
        .unwrap()
}

#[tokio::test]
async fn writes_on_a_apply_on_b_with_identical_rows() {
    let a = Node::open().await;
    let b = Node::open().await;

    a.exec("INSERT INTO sessions (id, title, language) VALUES ('s1', 'Standup', 'en')")
        .await;
    a.exec("INSERT INTO tags (id, name) VALUES ('t1', 'work')")
        .await;
    a.exec("INSERT INTO session_tags (id, session_id, tag_id) VALUES ('st1', 's1', 't1')")
        .await;
    a.exec("UPDATE sessions SET title = 'Daily standup' WHERE id = 's1'")
        .await;

    b.pull_from(&a).await;

    for table in ["sessions", "tags", "session_tags"] {
        let rows = a.snapshot(table).await;
        assert_eq!(rows.len(), 1, "{table} on A");
        assert_eq!(b.snapshot(table).await, rows, "{table} must match on B");
    }
    assert_eq!(b.title("s1").await, "Daily standup");

    // Hard delete travels as a tombstone.
    a.exec("DELETE FROM session_tags WHERE id = 'st1'").await;
    b.pull_from(&a).await;
    assert!(b.snapshot("session_tags").await.is_empty());

    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn concurrent_edits_to_one_column_converge() {
    let a = Node::open().await;
    let b = Node::open().await;
    a.exec("INSERT INTO sessions (id, title) VALUES ('s1', 'original')")
        .await;
    b.pull_from(&a).await;

    a.exec("UPDATE sessions SET title = 'from A' WHERE id = 's1'")
        .await;
    b.exec("UPDATE sessions SET title = 'from B' WHERE id = 's1'")
        .await;
    // Different column on B only: must survive the title conflict.
    b.exec("UPDATE sessions SET language = 'de' WHERE id = 's1'")
        .await;

    let a_page = a.serve_all(WireCursor::default(), &b).await;
    let b_page = b.serve_all(WireCursor::default(), &a).await;
    a.source.apply(b_page.changes).await.unwrap();
    b.source.apply(a_page.changes).await.unwrap();

    let winner = a.title("s1").await;
    assert_eq!(b.title("s1").await, winner, "LWW must converge");
    assert!(winner == "from A" || winner == "from B", "got {winner:?}");
    assert_eq!(a.snapshot("sessions").await, b.snapshot("sessions").await);
    let language: String = sqlx::query_scalar("SELECT language FROM sessions WHERE id = 's1'")
        .fetch_one(a.db.pool())
        .await
        .unwrap();
    assert_eq!(language, "de", "non-conflicting column merges per column");

    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn reapplying_a_page_is_a_no_op() {
    let a = Node::open().await;
    let b = Node::open().await;
    a.exec("INSERT INTO sessions (id, title) VALUES ('s1', 'one')")
        .await;
    a.exec("INSERT INTO tags (id, name) VALUES ('t1', 'x')")
        .await;

    let page = a.serve_all(WireCursor::default(), &b).await;
    b.source.apply(page.changes.clone()).await.unwrap();
    let rows = b.snapshot("sessions").await;
    let clocks = b.clock_state().await;
    assert!(!clocks.is_empty());

    b.source.apply(page.changes).await.unwrap();
    assert_eq!(b.snapshot("sessions").await, rows);
    assert_eq!(
        b.clock_state().await,
        clocks,
        "re-apply must not create, bump or reorder any change"
    );

    // An empty page is accepted and changes nothing.
    b.source.apply(Vec::new()).await.unwrap();
    assert_eq!(b.clock_state().await, clocks);

    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn requesters_own_changes_are_excluded() {
    let a = Node::open().await;
    let b = Node::open().await;
    a.exec("INSERT INTO sessions (id, title) VALUES ('from-a', 'a')")
        .await;
    b.pull_from(&a).await;
    b.exec("INSERT INTO sessions (id, title) VALUES ('from-b', 'b')")
        .await;

    let a_site = a.site().await;
    let to_a = b
        .source
        .changes_since(WireCursor::default(), Some(a_site.clone()), 1000)
        .await
        .unwrap();
    assert!(!to_a.changes.is_empty());
    assert!(
        to_a.changes.iter().all(|c| c.site_id != a_site),
        "A must never be served its own writes back"
    );

    // Without exclusion B relays A's changes too (a third device needs them).
    let all = b
        .source
        .changes_since(WireCursor::default(), None, 1000)
        .await
        .unwrap();
    assert!(all.changes.iter().any(|c| c.site_id == a_site));
    assert!(all.changes.len() > to_a.changes.len());

    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn pages_tile_the_stream_and_report_more_exactly() {
    let a = Node::open().await;
    let b = Node::open().await;
    for i in 0..5 {
        a.exec(&format!(
            "INSERT INTO tags (id, name) VALUES ('t{i}', 'n{i}')"
        ))
        .await;
    }
    // 5 rows × 6 non-PK columns.
    let total = a
        .source
        .changes_since(WireCursor::default(), None, 1000)
        .await
        .unwrap();
    assert_eq!(total.changes.len(), 30);
    assert!(!total.more);

    let exact = a
        .source
        .changes_since(WireCursor::default(), None, 30)
        .await
        .unwrap();
    assert!(!exact.more, "a page ending exactly at the tail has no more");

    let first = a
        .source
        .changes_since(WireCursor::default(), None, 29)
        .await
        .unwrap();
    assert!(first.more);
    let rest = a.source.changes_since(first.next, None, 29).await.unwrap();
    assert_eq!(rest.changes.len(), 1);
    assert!(!rest.more);
    assert_eq!(rest.next, total.next);

    let drained = a.source.changes_since(total.next, None, 0).await.unwrap();
    assert!(drained.changes.is_empty());
    assert_eq!(drained.next, total.next, "empty page keeps the cursor");

    // limit 0 is clamped to one row, never an empty page with more == true.
    let one = a
        .source
        .changes_since(WireCursor::default(), None, 0)
        .await
        .unwrap();
    assert_eq!(one.changes.len(), 1);

    b.pull_from(&a).await;
    assert_eq!(a.snapshot("tags").await, b.snapshot("tags").await);

    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn peer_cursor_survives_reopen_and_is_never_replicated() {
    let a = Node::open().await;
    assert_eq!(
        a.source.peer_cursor("peer-x").await.unwrap(),
        WireCursor::default()
    );
    a.source
        .set_peer_cursor(
            "peer-x",
            WireCursor {
                db_version: 7,
                seq: 2,
            },
        )
        .await
        .unwrap();
    a.source
        .set_peer_cursor(
            "peer-x",
            WireCursor {
                db_version: 9,
                seq: 0,
            },
        )
        .await
        .unwrap();
    a.source
        .set_peer_cursor(
            "peer-y",
            WireCursor {
                db_version: 1,
                seq: 4,
            },
        )
        .await
        .unwrap();

    let served = a
        .source
        .changes_since(WireCursor::default(), None, 1000)
        .await
        .unwrap();
    assert!(
        served
            .changes
            .iter()
            .all(|c| c.table != "sync_peer_cursors"),
        "cursors are device-local bookkeeping"
    );
    assert!(
        !hypr_crsqlite::is_crr(a.db.pool(), "sync_peer_cursors")
            .await
            .unwrap()
    );

    let path = a.path.clone();
    let dir = a._dir;
    a.db.close().await;
    drop(a.source);

    let db = Arc::new(open_db(&path).await);
    let source = CrsqliteChangeSource::new(Arc::clone(&db));
    assert_eq!(
        source.peer_cursor("peer-x").await.unwrap(),
        WireCursor {
            db_version: 9,
            seq: 0
        }
    );
    assert_eq!(
        source.peer_cursor("peer-y").await.unwrap(),
        WireCursor {
            db_version: 1,
            seq: 4
        }
    );
    db.close().await;
    drop(dir);
}

#[tokio::test]
async fn rows_written_before_sync_was_enabled_are_served() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let db = Arc::new(open_db(&path).await);
    sqlx::query("INSERT INTO sessions (id, title) VALUES ('old', 'pre-sync')")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(!hypr_crsqlite::is_crr(db.pool(), "sessions").await.unwrap());

    enable_crr_tables(&db).await.unwrap();
    enable_crr_tables(&db).await.unwrap();
    for table in hypr_db_app::SYNCED_TABLES {
        assert!(
            hypr_crsqlite::is_crr(db.pool(), table).await.unwrap(),
            "{table}"
        );
    }
    for table in hypr_db_app::NO_GO_TABLES
        .iter()
        .chain(hypr_db_app::ELIGIBLE_PENDING_PROOF)
    {
        assert!(
            !hypr_crsqlite::is_crr(db.pool(), table).await.unwrap(),
            "{table} must stay a plain table"
        );
    }

    let source = CrsqliteChangeSource::new(Arc::clone(&db));
    let page = source
        .changes_since(WireCursor::default(), None, 1000)
        .await
        .unwrap();
    assert!(
        page.changes.iter().any(|c| c.table == "sessions"
            && c.cid == "title"
            && c.val == sync_p2p::WireValue::Text("pre-sync".into())),
        "pre-existing rows must be backfilled into the change stream"
    );

    let b = Node::open().await;
    b.source.apply(page.changes).await.unwrap();
    assert_eq!(b.title("old").await, "pre-sync");

    db.close().await;
    b.close().await;
}
