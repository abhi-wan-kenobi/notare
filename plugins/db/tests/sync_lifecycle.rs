//! SYNC-5 lifecycle ordering: the start and stop sequences are load-bearing,
//! and nothing in the type system enforces them. This pins both.
//!
//! **Startup** (agent → env vars → configure → start): the C layer reads
//! `NOTARE_SYNC_AGENT_ADDR` + `NOTARE_SYNC_TOKEN` on every network call, so a
//! `cloudsync_start` that runs before the agent is up — or before the env
//! vars are published — makes the first background `network_sync` tick fail
//! against a dead socket, which kills the background loop (it breaks on any
//! non-transient error). The test pins the *observable* consequence: after
//! `SyncLifecycle::start`, an immediate `trigger()` succeeds end-to-end, which
//! is only possible if the agent was already serving and the env vars were
//! already published.
//!
//! **Teardown** (#101): `cloudsync_stop` must run before `pool().close()`.
//! The extension holds prepared statements against pool connections; closing
//! the pool first would finalize them against freed handles. After
//! `shutdown()`, status must report `network_initialized: false` (the flag
//! `cloudsync_stop` only clears on a successful cleanup+terminate) and the
//! pool must be closed.
//!
//! Env vars are process-global, so the whole file serializes on one static
//! lock; tests run in the same binary can never interleave.
//!
//! Requires the `sync` feature (linux/x86_64) — see the crate's Cargo.toml.

#![cfg(all(feature = "sync", target_os = "linux"))]

use std::sync::Arc;
use std::sync::OnceLock;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};
use sync_p2p::{Identity, P2pAgent, PeerStore, register_direct_addr};
use tauri_plugin_db::sync::SyncLifecycle;

/// Serialize env-var publication across all tests in this binary.
fn lifecycle_env_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// A tempdir-rooted identity + agent, so the test never touches the real
/// `<data_dir>/notare/sync/` device key or allowlist.
async fn test_agent() -> (tempfile::TempDir, P2pAgent) {
    let dir = tempfile::tempdir().unwrap();
    let identity = Identity::load_or_create_in(dir.path()).unwrap();
    let peers = PeerStore::load_or_create_in(dir.path()).unwrap();
    let agent = P2pAgent::start_with(identity, peers).await.unwrap();
    register_direct_addr(agent.node_id(), agent.direct_addresses()).await;
    (dir, agent)
}

/// A cloudsync-enabled db on a tempdir file (the same open options the app
/// uses when the `sync` feature is on).
async fn test_db(dir: &tempfile::TempDir) -> Arc<Db> {
    let db_path = dir.path().join("lifecycle.db");
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Local(&db_path),
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    hypr_db_app::prepare_schema(&db).await.unwrap();
    Arc::new(db)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_runs_the_agent_before_cloudsync_and_stop_shuts_down_in_order() {
    let _env = lifecycle_env_lock().lock().await;

    // ---- startup: agent up, then cloudsync on top of it ----
    let (agent_dir, agent) = test_agent().await;
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;

    let lifecycle = SyncLifecycle::start_with(Arc::clone(&db), agent).await.unwrap();

    // The agent was already live when cloudsync started, so an immediate
    // trigger must succeed through it end-to-end. This is the observable
    // consequence of "agent before cloudsync_start": a check against a dead
    // socket would fail the trigger.
    let received = lifecycle.trigger().await.unwrap();
    assert_eq!(
        received, 0,
        "first trigger after start must succeed (nothing pending yet)"
    );

    // Status pins the whole startup state.
    let status = lifecycle.status().await.unwrap();
    assert!(status.cloudsync_enabled, "extension loaded");
    assert!(status.configured, "runtime configured");
    assert!(status.running, "background loop running");
    assert!(status.network_initialized, "network_init ran");
    assert_eq!(status.last_error, None, "no error after startup");

    // ---- teardown: #101 order ----
    lifecycle.db_cloudsync_stop().await.unwrap();

    let status = lifecycle.status().await.unwrap();
    assert!(
        !status.running,
        "cloudsync_stop must stop the background loop"
    );
    assert!(
        !status.network_initialized,
        "cloudsync_stop must run cleanup+terminate (clears network_initialized)"
    );

    // cloudsync_stop ran before the pool close — now close it.
    db.pool().close().await;
    // The agent is stopped last.
    lifecycle.stop_agent().await.unwrap();

    drop(agent_dir);
    drop(db_dir);
}