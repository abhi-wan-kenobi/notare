//! S1 GO/NO-GO proof: two independent sqlite databases, in two independent
//! sqlx pools (two independent cloudsync site IDs), converge over the custom
//! P2P transport with **no** SQLite Cloud / Postgres / Supabase server.
//!
//! The CloudSync network layer in `crates/cloudsync/build/network_p2p.c` (built
//! into `cloudsync.so` under `cloudsync/from-source`) routes the core's
//! upload/check/apply/status calls to a [`sync_p2p::Broker`] over framed TCP
//! on localhost. The broker serves the CloudSync control protocol directly,
//! collapsing the S3 3-step flow onto an in-memory object store.
//!
//! Run: `cargo run -p sync-p2p --example sync_two_nodes`
//!
//! A green run = GO.

use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::Broker;

/// Shared managed-database ID so both sites address the same "database" on the
/// broker. (In production this is the SQLite Cloud database ID.)
const DB_ID: &str = "notare-spike";

/// Open a file-backed sqlite pool with a single connection (cloudsync context
/// is per-connection — init/enable/triggers must all run on the same handle),
/// load the from-source cloudsync extension (built with the P2P network
/// layer), and enable cloudsync on a `notes` table pointed at the broker.
async fn setup_node(uri: &str, broker_addr: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
    // apply() wires the from-source .so path into the connection options.
    let (options, _ext_path) = cloudsync::apply(options).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    // Smoke-check the extension loaded with the vendored version.
    let version: String = sqlx::query_scalar("SELECT cloudsync_version()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, cloudsync::CLOUDSYNC_VERSION);

    // The user table must exist before cloudsync_init (it inspects the PK).
    // 'cls' (CausalLengthSet) is the supported CRDT algo; force=1 (3rd arg)
    // skips the integer-PK check for our `id INTEGER PRIMARY KEY`.
    sqlx::query("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_init('notes', 'cls', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("SELECT cloudsync_enable('notes')")
        .execute(&pool)
        .await
        .unwrap();

    // Point this site at the broker. cloudsync_network_init_custom(address, dbId)
    // builds endpoints as {address}/v2/cloudsync/databases/{dbId}/{siteId}/{action}.
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    pool
}

async fn count_notes(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn note_body(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT body FROM notes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::main]
async fn main() {
    // 1. Start the broker (the local "CloudSync + S3" server).
    let broker = Broker::start().await.unwrap();
    let broker_addr = broker.address();
    println!("[broker] listening at {broker_addr}");

    // 2. Two independent file-backed databases (independent site IDs).
    let tmp = tempfile::tempdir().unwrap();
    let a_uri = format!(
        "sqlite://{}?mode=rwc",
        tmp.path().join("node_a.db").display()
    );
    let b_uri = format!(
        "sqlite://{}?mode=rwc",
        tmp.path().join("node_b.db").display()
    );

    let a = setup_node(&a_uri, &broker_addr).await;
    let b = setup_node(&b_uri, &broker_addr).await;
    println!("[nodes] A and B initialized; cloudsync enabled on 'notes'");

    // 3. Write rows on A.
    sqlx::query("INSERT INTO notes (id, body) VALUES (1, 'hello from A')")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notes (id, body) VALUES (2, 'second from A')")
        .execute(&a)
        .await
        .unwrap();
    println!("[A] wrote 2 rows");

    // 4. Sync A → broker (send+check), then B ← broker (send+check).
    let send: String = sqlx::query_scalar("SELECT cloudsync_network_sync()")
        .fetch_one(&a)
        .await
        .unwrap();
    println!("[A] sync -> broker: {send}");

    let recv: String = sqlx::query_scalar("SELECT cloudsync_network_check_changes()")
        .fetch_one(&b)
        .await
        .unwrap();
    println!("[B] check <- broker: {recv}");

    // 5. Assert convergence: B has A's rows.
    assert_eq!(count_notes(&b).await, 2, "B should have A's 2 rows");
    assert_eq!(note_body(&b, 1).await, "hello from A");
    assert_eq!(note_body(&b, 2).await, "second from A");
    println!("[conv] A -> B OK");

    // 6. Bidirectional: write on B, sync, A pulls.
    sqlx::query("INSERT INTO notes (id, body) VALUES (3, 'hello from B')")
        .execute(&b)
        .await
        .unwrap();
    println!("[B] wrote 1 row");

    let _ = sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
        .fetch_one(&b)
        .await
        .unwrap();
    let _ = sqlx::query_scalar::<_, String>("SELECT cloudsync_network_check_changes()")
        .fetch_one(&a)
        .await
        .unwrap();

    assert_eq!(count_notes(&a).await, 3, "A should have B's row");
    assert_eq!(note_body(&a, 3).await, "hello from B");
    println!("[conv] B -> A OK");

    // 7. Conflict-free concurrent update: both update row 1, sync until converged.
    sqlx::query("UPDATE notes SET body = 'A wins' WHERE id = 1")
        .execute(&a)
        .await
        .unwrap();
    sqlx::query("UPDATE notes SET body = 'B wins' WHERE id = 1")
        .execute(&b)
        .await
        .unwrap();
    println!("[both] updated row 1 concurrently");

    for _ in 0..3 {
        let _ = sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
            .fetch_one(&a)
            .await
            .unwrap();
        let _ = sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
            .fetch_one(&b)
            .await
            .unwrap();
    }
    let a_body = note_body(&a, 1).await;
    let b_body = note_body(&b, 1).await;
    assert_eq!(
        a_body, b_body,
        "row 1 converged (A={a_body:?}, B={b_body:?})"
    );
    println!("[conv] concurrent update converged (row 1 = {a_body:?} on both)");

    println!("\n=== S1 GO: two-node convergence over custom P2P transport ===");
    a.close().await;
    b.close().await;
    broker.stop().await;
}
