//! REGRESSION (SYNC-5 drain-check): the hub's `check` serves ONE blob per
//! call, and the C `cloudsync_network_sync` breaks out of its check loop on
//! the first `nrows > 0` — so a single call pulls at most ONE pending blob.
//! SYNC-4 found this as a divergence class: a caller that "syncs" once
//! silently stays behind when several blobs are pending.
//!
//! This pins the fix in `cloudsync::network_sync` (the Rust wrapper): it must
//! loop until the hub reports `receive.rows == 0`, so ONE call catches a site
//! up on ANY number of pending blobs.
//!
//! Two nodes over the real iroh P2P transport (A = hub/broker, B = spoke).
//! A writes two rows and pushes both blobs while B is idle, so B is two blobs
//! behind — one undrained sync call would leave B with exactly one row.
//!
//! Gated on `from-source` like the convergence examples: the custom P2P
//! network layer only exists in the from-source cloudsync build.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};

const DB_ID: &str = "notare-v06-drain";

async fn setup_node(uri: &str, broker_addr: &str, local_agent_tcp: &str, token: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(uri).unwrap();
    let (options, _ext_path) = cloudsync::apply(options).unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

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
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(broker_addr)
        .bind(DB_ID)
        .execute(&pool)
        .await
        .unwrap();

    // SAFETY: sequential single-process test; each sync call re-sets both env
    // vars immediately before use, and no other thread reads them concurrently.
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

/// The wrapper under test — one call must catch the site fully up.
async fn sync_once(pool: &SqlitePool, local_agent_tcp: &str, token: &str) -> i64 {
    // SAFETY: see setup_node.
    unsafe {
        std::env::set_var("NOTARE_SYNC_AGENT_ADDR", local_agent_tcp);
        std::env::set_var("NOTARE_SYNC_TOKEN", token);
    }
    cloudsync::network_sync(pool, None, None).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn network_sync_drains_all_pending_blobs_in_one_call() {
    let tmp = tempfile::tempdir().unwrap();

    let dir_a = tempfile::tempdir_in(tmp.path()).unwrap();
    let dir_b = tempfile::tempdir_in(tmp.path()).unwrap();

    let id_a = Identity::load_or_create_in(dir_a.path()).unwrap();
    let id_b = Identity::load_or_create_in(dir_b.path()).unwrap();

    let peers_a = PeerStore::load_or_create_in(dir_a.path()).unwrap();
    let peers_b = PeerStore::load_or_create_in(dir_b.path()).unwrap();
    peers_a.add_peer(id_b.id(), "Node B").unwrap();
    peers_b.add_peer(id_a.id(), "Node A (hub)").unwrap();

    let agent_a = P2pAgent::start_with(id_a, peers_a).await.unwrap();
    let agent_b = P2pAgent::start_with(id_b, peers_b).await.unwrap();

    register_direct_addr(agent_a.node_id(), agent_a.direct_addresses()).await;
    register_direct_addr(agent_b.node_id(), agent_b.direct_addresses()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let a_tcp = agent_a.local_addr.clone();
    let b_tcp = agent_b.local_addr.clone();
    let hub = agent_a.address();

    let a = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_a.path().join("node_a.db").display()
        ),
        &hub,
        &a_tcp,
        agent_a.token(),
    )
    .await;
    let b = setup_node(
        &format!(
            "sqlite://{}?mode=rwc",
            dir_b.path().join("node_b.db").display()
        ),
        &hub,
        &b_tcp,
        agent_b.token(),
    )
    .await;

    // A writes TWO rows and pushes both while B is idle → B is two blobs
    // behind. An undrained sync pulls at most one blob (the C check loop
    // breaks on the first `nrows > 0`).
    sqlx::query("INSERT INTO notes (id, body) VALUES (1, 'first')")
        .execute(&a)
        .await
        .unwrap();
    sync_once(&a, &a_tcp, agent_a.token()).await;
    sqlx::query("INSERT INTO notes (id, body) VALUES (2, 'second')")
        .execute(&a)
        .await
        .unwrap();
    sync_once(&a, &a_tcp, agent_a.token()).await;

    // THE assertion: a single network_sync call on B must drain BOTH blobs.
    let received = sync_once(&b, &b_tcp, agent_b.token()).await;
    assert_eq!(
        received,
        2,
        "one network_sync call must drain all pending blobs (rows received)"
    );
    assert_eq!(
        count_notes(&b).await,
        2,
        "B must be fully caught up after a single drained sync call"
    );

    // Draining is idempotent: with nothing pending, another call receives 0.
    let received = sync_once(&b, &b_tcp, agent_b.token()).await;
    assert_eq!(received, 0, "no pending changes → 0 rows received");

    a.close().await;
    b.close().await;
    agent_a.stop().await;
    agent_b.stop().await;
}
