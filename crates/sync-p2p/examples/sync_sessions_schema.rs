//! SYNC-6 proof: the CRDT converges notare's **real** session schema —
//! TEXT-PK, `STRICT`, NOT-NULL-defaulted TEXT columns, a `deleted_at`
//! tombstone column, and a FOREIGN KEY between `session_documents` and
//! `sessions` — not the synthetic `notes (id INTEGER PRIMARY KEY, body TEXT)`
//! table the earlier proofs used.
//!
//! SYNC-5 wired the sync stack into the app but enabled zero tables, because
//! nothing had ever shown `cls` converging tables shaped like the real ones.
//! This is that proof. Same two-node iroh harness as `sync_two_nodes`
//! (agents, `register_direct_addr`, one elected broker on A); the only
//! differences are the schema under test and the scenarios.
//!
//! The table shapes mirror
//! `crates/db-app/migrations/20260710223922_canonical_data_model.sql`
//! (a representative subset of columns each):
//!
//! - `sessions` — `id TEXT PRIMARY KEY NOT NULL`, `title TEXT NOT NULL
//!   DEFAULT ''`, other NOT-NULL-defaulted TEXT columns, `deleted_at TEXT`,
//!   `STRICT`.
//! - `session_documents` — `id TEXT PRIMARY KEY NOT NULL`, `session_id TEXT`
//!   with `FOREIGN KEY (session_id) REFERENCES sessions(id)`, NOT-NULL-defaulted
//!   TEXT columns, `deleted_at TEXT`, `STRICT`. Foreign keys are enforced
//!   (`PRAGMA foreign_keys=ON`, as the real app does in `crates/db-core`).
//!
//! Run: `cargo run -p sync-p2p --example sync_sessions_schema --features from-source`
//!
//! A green run = the real tables converge, conflict-free, including soft
//! deletes via `deleted_at` and hard deletes through the CRDT tombstone.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

/// Shared managed-database ID so both sites address the same "database" on
/// their brokers.
const DB_ID: &str = "notare-v06";

/// Bound on drain/settle loops so a non-converging run fails fast and loudly
/// rather than spinning.
const MAX_DRAIN: usize = 16;

/// Representative `sessions` subset of the canonical data model. Column names,
/// types, NOT NULL, and defaults match the migration; only the column count is
/// reduced (a subset is fine per the spec — what matters is TEXT-PK + STRICT +
/// NOT-NULL-defaulted TEXT + `deleted_at`).
///
/// `updated_at`/`created_at` carry a runtime `DEFAULT`, which the cloudsync
/// init check (all NOT NULL non-PK columns must have a DEFAULT) requires and
/// the real table satisfies the same way.
const CREATE_SESSIONS: &str = "CREATE TABLE IF NOT EXISTS sessions (
  id           TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL DEFAULT '',
  title        TEXT NOT NULL DEFAULT '',
  kind         TEXT NOT NULL DEFAULT 'meeting',
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at   TEXT
) STRICT";

/// Representative `session_documents` subset, with the real FK to `sessions`.
const CREATE_SESSION_DOCUMENTS: &str = "CREATE TABLE IF NOT EXISTS session_documents (
  id          TEXT PRIMARY KEY NOT NULL,
  session_id  TEXT NOT NULL DEFAULT '',
  kind        TEXT NOT NULL DEFAULT 'note',
  title       TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',
  sort_order  INTEGER NOT NULL DEFAULT 0,
  deleted_at  TEXT,
  FOREIGN KEY (session_id) REFERENCES sessions (id)
) STRICT";

async fn setup_node(uri: &str, broker_addr: &str, local_agent_tcp: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
    // The real app enforces FKs on every connection (crates/db-core sets
    // PRAGMA foreign_keys=ON), so the proof must pay the same price — the FK
    // must hold on *both* nodes, including on rows that arrive via sync.
    let options = options.pragma("foreign_keys", "ON");
    let (options, _ext_path) = cloudsync::apply(options).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    let version: String = sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, cloudsync::CLOUDSYNC_VERSION);

    sqlx::query(CREATE_SESSIONS).execute(&pool).await.unwrap();
    sqlx::query(CREATE_SESSION_DOCUMENTS)
        .execute(&pool)
        .await
        .unwrap();
    for table in ["sessions", "session_documents"] {
        sqlx::query("SELECT cloudsync_init(?, 'cls', 1)")
            .bind(table)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_enable(?)")
            .bind(table)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    // SAFETY: sequential single-process example; every sync call re-sets this
    // immediately before use, and no other thread reads it concurrently.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }

    pool
}

async fn run_sync(pool: &SqlitePool, local_agent_tcp: &str) -> String {
    // SAFETY: see setup_node.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_check(pool: &SqlitePool, local_agent_tcp: &str) -> String {
    // SAFETY: see setup_node.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_check_changes()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// `rows` from a `check` reply (`{"receive":{"rows":N,...}}`), parsed with
/// serde. `None` (unreadable reply) is deliberately distinct from `Some(0)`
/// (nothing pending) — conflating them is the silent-divergence bug the
/// three-node proof's `drain_check` exists to prevent.
fn rows_received(resp: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    v.get("receive")?.get("rows")?.as_u64()
}

/// Pull until the hub has nothing left for this site (the hub serves one
/// blob per `check` — see docs/internal/sync-p2p.md §15.1).
async fn drain_check(pool: &SqlitePool, tcp: &str, label: &str) -> usize {
    let mut applied = 0;
    for _ in 0..MAX_DRAIN {
        let resp = run_check(pool, tcp).await;
        match rows_received(&resp) {
            None => panic!(
                "[{label}] unreadable check reply, cannot know if changes are pending: {resp}"
            ),
            Some(0) => {
                println!("[{label}] drained {applied} change set(s)");
                return applied;
            }
            Some(_) => applied += 1,
        }
    }
    panic!("[{label}] hit MAX_DRAIN ({MAX_DRAIN}) with changes still pending");
}

/// Push local changes, then drain everything pending for this site.
async fn sync_and_drain(pool: &SqlitePool, tcp: &str, label: &str) {
    run_sync(pool, tcp).await;
    drain_check(pool, tcp, label).await;
}

async fn session_title(pool: &SqlitePool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn session_deleted_at(pool: &SqlitePool, id: &str) -> Option<Option<String>> {
    sqlx::query_scalar("SELECT deleted_at FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn doc_row(pool: &SqlitePool, id: &str) -> Option<(String, String, Option<String>)> {
    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT session_id, title, deleted_at FROM session_documents WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap();
    row
}

async fn count(pool: &SqlitePool, table: Table) -> i64 {
    let sql = match table {
        Table::Sessions => "SELECT COUNT(*) FROM sessions",
        Table::SessionDocuments => "SELECT COUNT(*) FROM session_documents",
    };
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

enum Table {
    Sessions,
    SessionDocuments,
}

fn short(agent: &P2pAgent) -> String {
    agent.node_id().to_z32().chars().take(8).collect()
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let tmp = tempfile::tempdir().unwrap();

    // 1. Two agents, each in its own data dir, each allowlisting the other.
    let dir_a = tempfile::tempdir_in(tmp.path()).unwrap();
    let dir_b = tempfile::tempdir_in(tmp.path()).unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_a.add_peer(id_b.id(), "Node B").unwrap();
    peers_b.add_peer(id_a.id(), "Node A").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();

    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let a_tcp = agent_a.local_addr.clone();
    let b_tcp = agent_b.local_addr.clone();
    let broker = agent_a.address(); // A hosts the shared broker

    println!("[agents] A={} (broker, tcp {a_tcp})", short(&agent_a));
    println!("[agents] B={} (tcp {b_tcp})", short(&agent_b));
    println!("[peers]  A allowlists B; B allowlists A");

    // 2. Two independent file-backed databases, each with the real session
    //    schema. FKs are ON on both — a child whose parent never arrived must
    //    fail loudly, on either node.
    let a = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_a.path().join("node_a.db").display()
        ),
        &broker,
        &a_tcp,
    )
    .await;
    let b = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_b.path().join("node_b.db").display()
        ),
        &broker,
        &b_tcp,
    )
    .await;
    println!(
        "[nodes] A and B initialized; cloudsync enabled on sessions + session_documents (broker = A)"
    );

    // 3. Scenario 1 — A -> B converge, with the FK live. A inserts a session
    //    and a child session_document (TEXT-PK uuids); B must end up with
    //    both rows, identical values, session_id intact.
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title, kind)
         VALUES ('11111111-1111-1111-1111-111111111111', 'ws-a', 'A writes first', 'meeting')",
    )
    .execute(&a)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, kind, title, body, sort_order)
         VALUES ('22222222-2222-2222-2222-222222222222',
                 '11111111-1111-1111-1111-111111111111',
                 'note', 'child of A', '{\"doc\":true}', 0)",
    )
    .execute(&a)
    .await
    .unwrap();
    println!("[A] wrote session + child document (FK live)");

    run_sync(&a, &a_tcp).await;
    drain_check(&b, &b_tcp, "B").await;

    assert_eq!(
        session_title(&b, "11111111-1111-1111-1111-111111111111").await,
        Some("A writes first".into()),
        "B has A's session"
    );
    assert_eq!(
        doc_row(&b, "22222222-2222-2222-2222-222222222222").await,
        Some((
            "11111111-1111-1111-1111-111111111111".into(),
            "child of A".into(),
            None
        )),
        "B has A's child document with the FK intact"
    );
    println!("[conv] A -> B OK (TEXT-PK rows, STRICT types, FK intact)");

    // 4. Scenario 2 — B -> A converge, the reverse direction.
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, title, kind)
         VALUES ('33333333-3333-3333-3333-333333333333', 'ws-b', 'B writes second', 'meeting')",
    )
    .execute(&b)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, kind, title, body, sort_order)
         VALUES ('44444444-4444-4444-4444-444444444444',
                 '33333333-3333-3333-3333-333333333333',
                 'note', 'child of B', '{\"doc\":true}', 0)",
    )
    .execute(&b)
    .await
    .unwrap();
    println!("[B] wrote session + child document");

    run_sync(&b, &b_tcp).await;
    drain_check(&a, &a_tcp, "A").await;

    assert_eq!(
        session_title(&a, "33333333-3333-3333-3333-333333333333").await,
        Some("B writes second".into()),
        "A has B's session"
    );
    assert_eq!(
        doc_row(&a, "44444444-4444-4444-4444-444444444444").await,
        Some((
            "33333333-3333-3333-3333-333333333333".into(),
            "child of B".into(),
            None
        )),
        "A has B's child document with the FK intact"
    );
    println!("[conv] B -> A OK");

    // 5. Scenario 3 — concurrent update, conflict-free. Both nodes UPDATE the
    //    same session's `title` while neither has synced; after reconnect all
    //    nodes hold ONE of the two writes — never torn or merged garbage.
    sqlx::query("UPDATE sessions SET title = 'A renamed it' WHERE id = '11111111-1111-1111-1111-111111111111'")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET title = 'B renamed it' WHERE id = '11111111-1111-1111-1111-111111111111'")
        .execute(&b)
        .await
        .unwrap();
    println!("[both] updated session 1111…'s title concurrently while disconnected");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, "A").await;
        sync_and_drain(&b, &b_tcp, "B").await;
        let (ta, tb) = (
            session_title(&a, "11111111-1111-1111-1111-111111111111").await,
            session_title(&b, "11111111-1111-1111-1111-111111111111").await,
        );
        if ta == tb {
            // Agreement is not yet convergence: an intermediate value can
            // match on both while a pending blob would still move one. Settle
            // one more round and require the value to be unchanged.
            sync_and_drain(&a, &a_tcp, "A").await;
            sync_and_drain(&b, &b_tcp, "B").await;
            let (fa, fb) = (
                session_title(&a, "11111111-1111-1111-1111-111111111111").await,
                session_title(&b, "11111111-1111-1111-1111-111111111111").await,
            );
            assert_eq!(fa, fb, "title diverged A vs B after a settling round");
            assert_eq!(
                fa, ta,
                "title was not stable — agreed on {ta:?} then moved to {fa:?}"
            );
            let settled = fa.expect("row vanished during settle");
            assert!(
                settled == "A renamed it" || settled == "B renamed it",
                "converged value {settled:?} is neither of the two writes — torn or merged"
            );
            println!("[conv] concurrent update converged and held (title = {settled:?} on both)");
            break;
        }
    }
    let final_title = session_title(&a, "11111111-1111-1111-1111-111111111111").await;
    assert_eq!(
        session_title(&b, "11111111-1111-1111-1111-111111111111").await,
        final_title,
        "A and B must agree on the settled title"
    );

    // 6. Scenario 4 — tombstone as delete (v0.6 gate-critical). A soft-deletes
    //    a session (`deleted_at`, an ordinary column update — what the trash
    //    view reads) and hard-DELETEs the child row (which drops a CRDT
    //    tombstone through the core's delete trigger). B must see the exact
    //    `deleted_at` value and the row must NOT resurrect.
    const RIP_DOC: &str = "22222222-2222-2222-2222-222222222222";
    sqlx::query("UPDATE sessions SET deleted_at = '2026-08-30T00:00:00Z', updated_at = '2026-08-30T00:00:00Z' WHERE id = '11111111-1111-1111-1111-111111111111'")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("DELETE FROM session_documents WHERE id = ?")
        .bind(RIP_DOC)
        .execute(&a)
        .await
        .unwrap();
    println!("[A] soft-deleted session 1111… (deleted_at) and hard-deleted its child document");

    run_sync(&a, &a_tcp).await;
    drain_check(&b, &b_tcp, "B").await;

    assert_eq!(
        session_deleted_at(&b, "11111111-1111-1111-1111-111111111111").await,
        Some(Some("2026-08-30T00:00:00Z".into())),
        "the deleted_at tombstone value must sync across verbatim"
    );
    assert!(
        doc_row(&b, RIP_DOC).await.is_none(),
        "the deleted child row must not resurrect on B"
    );
    assert!(
        session_deleted_at(&b, "11111111-1111-1111-1111-111111111111")
            .await
            .is_some(),
        "the soft-deleted session row itself must survive (trash view reads it)"
    );
    println!("[conv] tombstone-as-delete OK (deleted_at synced, child gone, row survived)");

    // 6b. No resurrection under further sync traffic. Run several more
    //     rounds — if the tombstone or the delete ever loses to an older
    //     concurrent change, this is where it shows.
    for _ in 0..3 {
        sync_and_drain(&a, &a_tcp, "A").await;
        sync_and_drain(&b, &b_tcp, "B").await;
    }
    assert!(
        doc_row(&b, RIP_DOC).await.is_none(),
        "deleted child row resurrected on B after further syncs"
    );
    assert!(
        doc_row(&a, RIP_DOC).await.is_none(),
        "deleted child row resurrected on A after further syncs"
    );
    assert_eq!(
        session_deleted_at(&b, "11111111-1111-1111-1111-111111111111").await,
        Some(Some("2026-08-30T00:00:00Z".into())),
        "deleted_at tombstone changed after further syncs"
    );
    println!("[conv] no resurrection after further sync rounds");

    // 7. Scenario 5 — multi-row catch-up. Both nodes write several rows
    //    across both tables before any drain; afterwards both nodes must hold
    //    the full union. This exercises the SYNC-5 drain fix: each `check`
    //    applies one change set, so multiple blobs must all be drained.
    for n in 0..3 {
        let id = format!("aaaa0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, ?)")
            .bind(&id)
            .bind(format!("bulk session {n} from A"))
            .execute(&a)
            .await
            .unwrap();
    }
    for n in 0..3 {
        let id = format!("bbbb0000-0000-0000-0000-{n:012x}");
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, title, body)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind("aaaa0000-0000-0000-0000-000000000000")
        .bind(format!("bulk doc {n} from A"))
        .bind("{}")
        .execute(&a)
        .await
        .unwrap();
    }
    for n in 0..3 {
        let id = format!("cccc0000-0000-0000-0000-{n:012x}");
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, ?)")
            .bind(&id)
            .bind(format!("bulk session {n} from B"))
            .execute(&b)
            .await
            .unwrap();
    }
    println!("[both] wrote 3 sessions + 3 docs (A) and 3 sessions (B) before draining");

    for _ in 0..MAX_DRAIN {
        sync_and_drain(&a, &a_tcp, "A").await;
        sync_and_drain(&b, &b_tcp, "B").await;
    }

    assert_eq!(
        count(&a, Table::Sessions).await,
        count(&b, Table::Sessions).await,
        "session count differs A vs B"
    );
    assert_eq!(
        count(&a, Table::SessionDocuments).await,
        count(&b, Table::SessionDocuments).await,
        "document count differs A vs B"
    );
    assert_eq!(
        count(&a, Table::Sessions).await,
        2 + 3 + 3,
        "expected 8 sessions on A"
    );
    assert_eq!(
        count(&a, Table::SessionDocuments).await,
        2 + 3 - 1,
        "expected 4 documents on A (one deleted)"
    );
    assert_eq!(
        session_title(&b, "aaaa0000-0000-0000-0000-000000000002").await,
        Some("bulk session 2 from A".into()),
        "B is missing one of A's bulk rows (drain did not catch up)"
    );
    assert_eq!(
        session_title(&a, "cccc0000-0000-0000-0000-000000000002").await,
        Some("bulk session 2 from B".into()),
        "A is missing one of B's bulk rows (drain did not catch up)"
    );
    println!("[conv] multi-row catch-up OK (full set equality across both tables)");

    // Whole-schema agreement, not just the rows each scenario touched.
    for id in [
        "11111111-1111-1111-1111-111111111111",
        "33333333-3333-3333-3333-333333333333",
        "aaaa0000-0000-0000-0000-000000000000",
        "aaaa0000-0000-0000-0000-000000000001",
        "aaaa0000-0000-0000-0000-000000000002",
        "cccc0000-0000-0000-0000-000000000000",
        "cccc0000-0000-0000-0000-000000000001",
        "cccc0000-0000-0000-0000-000000000002",
    ] {
        assert_eq!(
            session_title(&a, id).await,
            session_title(&b, id).await,
            "session {id} differs A vs B"
        );
    }
    for id in [
        "44444444-4444-4444-4444-444444444444",
        "bbbb0000-0000-0000-0000-000000000000",
        "bbbb0000-0000-0000-0000-000000000001",
        "bbbb0000-0000-0000-0000-000000000002",
    ] {
        assert_eq!(
            doc_row(&a, id).await,
            doc_row(&b, id).await,
            "document {id} differs A vs B"
        );
    }

    println!(
        "\n=== SYNC-6 schema proof: real sessions + session_documents converge (TEXT-PK, STRICT, FK, tombstones) ==="
    );

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}
