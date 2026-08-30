//! v0.6 convergence proof: two independent sqlite databases, in two
//! independent sqlx pools (two independent cloudsync site IDs), converge
//! over an **iroh/QUIC P2P transport** with **no** SQLite Cloud / Postgres /
//! Supabase server.
//!
//! Each node runs its own [`sync_p2p::P2pAgent`] (iroh endpoint + local
//! broker + peer allowlist). The CloudSync network layer in
//! `crates/cloudsync/build/network_p2p.c` routes the core's
//! upload/check/apply/status calls to its local agent over framed TCP on
//! 127.0.0.1; the agent dials the other node over iroh, enforcing the peer
//! allowlist at dial + accept (closing the §12 SSRF finding). The broker on
//! each node serves the CloudSync control protocol directly, collapsing the
//! S3 3-step flow onto an in-memory object store.
//!
//! Run: `cargo run -p sync-p2p --example sync_two_nodes --features from-source`
//!
//! A green run = the iroh transport converges, conflict-free.

use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

use std::time::Duration;

/// Shared managed-database ID so both sites address the same "database" on
/// their brokers. (In production this is the SQLite Cloud database ID.)
const DB_ID: &str = "notare-v06";

/// Open a file-backed sqlite pool with a single connection (cloudsync context
/// is per-connection — init/enable/triggers must all run on the same handle),
/// load the from-source cloudsync extension (built with the P2P network
/// layer), and enable cloudsync on a `notes` table pointed at a shared broker
/// peer.
///
/// `broker_addr` is the `p2p://<node-id>` address of the **shared broker**
/// peer both sites sync through (CloudSync's protocol assumes a shared server:
/// both sites push to and pull from the same broker). In this proof node A
/// hosts the shared broker; B reaches it over iroh. `local_agent_tcp` is this
/// node's own agent's local TCP address, which the C layer reaches via
/// `NOTARE_SYNC_AGENT_ADDR`.
async fn setup_node(
    uri: &str,
    broker_addr: &str,
    local_agent_tcp: &str,
    token: &str,
) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
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

    // Point this site at the shared broker peer's node id.
    // cloudsync_network_init_custom builds endpoints as
    // {address}/v2/cloudsync/databases/{dbId}/{siteId}/{action} — address is
    // "p2p://<broker-node-id-fingerprint>", so every endpoint the core hands
    // the C layer names the broker by node id, which the local agent routes
    // over iroh (or serves locally if it IS the broker).
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    // The C layer reads the local agent's TCP address from this env var on
    // every call. Set it to THIS node's agent before any sync call below.
    // (Both nodes share one process; because the example syncs sequentially,
    // flipping the env var per-node is safe — see run_sync/run_check.)
    // SAFETY: no other thread reads/writes this env var concurrently — the
    // two nodes' sync calls are strictly sequential in this example, and the
    // C `getenv` read happens only during the blocking sync call below.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", token);
    }

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

/// Run a sync on `pool`'s node: set the C-facing env var to that node's agent
/// address, then call cloudsync_network_sync(). The C layer connects to the
/// local agent, which dials the peer over iroh.
async fn run_sync(pool: &SqlitePool, local_agent_tcp: &str, token: &str) -> String {
    // SAFETY: sequential single-process example; see setup_node note.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", token);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_sync()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn run_check(pool: &SqlitePool, local_agent_tcp: &str, token: &str) -> String {
    // SAFETY: sequential single-process example; see setup_node note.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", token);
    }
    sqlx::query_scalar::<_, String>("SELECT cloudsync_network_check_changes()")
        .fetch_one(pool)
        .await
        .unwrap()
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

    // Register direct addresses so each agent can dial the other without relay
    // (RelayMode::Disabled for the same-machine proof).
    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let a_tcp = agent_a.local_addr.clone();
    let b_tcp = agent_b.local_addr.clone();
    let a_addr = agent_a.address(); // p2p://<A-fingerprint> — the shared broker

    println!(
        "[agents] A={} (tcp {a_tcp})",
        agent_a
            .node_id()
            .to_z32()
            .chars()
            .take(8)
            .collect::<String>()
    );
    println!(
        "[agents] B={} (tcp {b_tcp})",
        agent_b
            .node_id()
            .to_z32()
            .chars()
            .take(8)
            .collect::<String>()
    );
    println!("[peers]  A allowlists B; B allowlists A");

    // 2. Two independent file-backed databases. Both point at A's node id as
    //    the shared broker (CloudSync's protocol assumes a shared server both
    //    sites push to and pull from). A serves its own broker locally; B
    //    reaches it over iroh. Each site still has its own site id, so the
    //    CRDT changesets stay per-site.
    let a_uri = format!(
        "sqlite://{}?mode=rwc",
        dir_a.path().join("node_a.db").display()
    );
    let b_uri = format!(
        "sqlite://{}?mode=rwc",
        dir_b.path().join("node_b.db").display()
    );

    let a = setup_node(&a_uri, &a_addr, &a_tcp, agent_a.token()).await;
    let b = setup_node(&b_uri, &a_addr, &b_tcp, agent_b.token()).await;
    println!("[nodes] A and B initialized; cloudsync enabled on 'notes' (broker = A)");

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

    // 4. Sync A → shared broker (A pushes its rows to A's own broker), then
    //    B ← shared broker over iroh (B pulls A's rows from A's broker).
    let send = run_sync(&a, &a_tcp, agent_a.token()).await;
    println!("[A] sync -> broker (local): {send}");
    let recv = run_check(&b, &b_tcp, agent_b.token()).await;
    println!("[B] check <- broker (iroh): {recv}");

    assert_eq!(count_notes(&b).await, 2, "B should have A's 2 rows");
    assert_eq!(note_body(&b, 1).await, "hello from A");
    assert_eq!(note_body(&b, 2).await, "second from A");
    println!("[conv] A -> B OK (over iroh)");

    // 5. Bidirectional: write on B, sync (B pushes to A's broker over iroh),
    //    A pulls from its own broker (local).
    sqlx::query("INSERT INTO notes (id, body) VALUES (3, 'hello from B')")
        .execute(&b)
        .await
        .unwrap();
    println!("[B] wrote 1 row");

    run_sync(&b, &b_tcp, agent_b.token()).await;
    run_check(&a, &a_tcp, agent_a.token()).await;

    assert_eq!(count_notes(&a).await, 3, "A should have B's row");
    assert_eq!(note_body(&a, 3).await, "hello from B");
    println!("[conv] B -> A OK (B pushed over iroh)");

    // 6. Conflict-free concurrent update: both update row 1, sync until converged.
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
        run_sync(&a, &a_tcp, agent_a.token()).await;
        run_sync(&b, &b_tcp, agent_b.token()).await;
    }
    let a_body = note_body(&a, 1).await;
    let b_body = note_body(&b, 1).await;
    assert_eq!(
        a_body, b_body,
        "row 1 converged (A={a_body:?}, B={b_body:?})"
    );
    println!("[conv] concurrent update converged (row 1 = {a_body:?} on both)");

    println!("\n=== SYNC-3 GO: two-node convergence over iroh P2P transport ===");

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}
