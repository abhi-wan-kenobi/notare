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
//! Requires the `sync` feature, on the app's sync-platform gate (§22) — see
//! the crate's Cargo.toml.

#![cfg(all(feature = "sync", sync_platform))]

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

    let lifecycle = SyncLifecycle::start_with(Arc::clone(&db), agent)
        .await
        .unwrap();

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

/// A second lifecycle on the same agent shape pins that `take()` in
/// `PluginDbRuntime::shutdown` hands the owned `Option` to the teardown steps
/// (auditors once read `guard.take()` as "discard" and claimed step 1 never
/// runs) — and exercises the `sync_this_device`/`sync_list_peers` fallbacks on
/// a runtime whose lifecycle is not up.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runtime_teardown_runs_every_step_and_fallbacks_answer_without_lifecycle() {
    let _env = lifecycle_env_lock().lock().await;

    let (agent_dir, agent) = test_agent().await;
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;
    let runtime = tauri_plugin_db::PluginDbRuntime::new(std::sync::Arc::clone(&db));

    // Before start_sync: the fallbacks answer from disk, not from a lifecycle.
    let device = runtime.sync_this_device().await.unwrap();
    assert!(
        !device.is_empty(),
        "fallback identity fingerprint should be a non-empty z32, got {device:?}"
    );
    assert!(
        runtime.sync_list_peers().await.is_empty(),
        "fresh allowlist has no peers"
    );

    runtime.start_sync_with(agent).await.unwrap();
    let status = runtime.sync_status().await.unwrap();
    assert!(status.running, "lifecycle started");

    runtime.shutdown().await;

    // shutdown ran cloudsync_stop before closing the pool: the db-level
    // status must show the stopped state, and the pool must be closed.
    let status = db.cloudsync_status().await.unwrap();
    assert!(
        !status.running && !status.network_initialized,
        "shutdown must have run cloudsync_stop (take() must hand the lifecycle to the steps, not discard it)"
    );

    drop(agent_dir);
    drop(db_dir);
}

/// SYNC-6: `SyncLifecycle::add_peer` must reject garbage input, must refuse
/// to pair a device with itself (the allowlist gates real peers, not a
/// self-loop), and must accept a fingerprint in either the grouped (dashed,
/// display) or compact (ungrouped) form — the UI shows grouped, but
/// `Fingerprint::parse` round-trips both, and `add_peer` must not narrow that.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn add_peer_rejects_invalid_fingerprint_and_self_but_accepts_grouped_or_compact() {
    let _env = lifecycle_env_lock().lock().await;

    let (agent_dir, agent) = test_agent().await;
    let own_fingerprint = sync_p2p::Fingerprint::from_pubkey(&agent.node_id())
        .as_str()
        .to_string();
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;
    let lifecycle = SyncLifecycle::start_with(Arc::clone(&db), agent)
        .await
        .unwrap();

    assert!(
        lifecycle
            .add_peer("not-a-real-fingerprint!!", "bad")
            .is_err(),
        "garbage input must be rejected"
    );

    assert!(
        lifecycle.add_peer(&own_fingerprint, "myself").is_err(),
        "a device must not be able to add itself as its own peer"
    );

    // Two distinct peer identities: one added via the grouped (dashed) form,
    // one via the compact (ungrouped) form.
    let peer_a_dir = tempfile::tempdir().unwrap();
    let peer_a = Identity::load_or_create_in(peer_a_dir.path()).unwrap();
    let peer_b_dir = tempfile::tempdir().unwrap();
    let peer_b = Identity::load_or_create_in(peer_b_dir.path()).unwrap();

    let grouped = peer_a.fingerprint().as_str().to_string();
    assert!(grouped.contains('-'), "sanity: grouped form is dashed");
    let resolved = lifecycle.add_peer(&grouped, "Peer A").unwrap();
    assert_eq!(
        resolved,
        peer_a.fingerprint().as_str(),
        "add_peer returns the resolved grouped fingerprint"
    );

    let compact = peer_b.fingerprint().compact();
    assert!(!compact.contains('-'), "sanity: compact form is ungrouped");
    lifecycle.add_peer(&compact, "Peer B").unwrap();

    let peers = lifecycle.list_peers();
    assert_eq!(peers.len(), 2, "both grouped- and compact-form adds landed");
    assert!(peers.iter().any(|p| p.node_id == peer_a.id()));
    assert!(peers.iter().any(|p| p.node_id == peer_b.id()));

    drop(agent_dir);
    drop(db_dir);
}

/// Runtime opt-out (the desktop `sync_enabled` setting toggled off while the
/// app keeps running): `PluginDbRuntime::stop_sync` must run the same
/// `db_cloudsync_stop` → `stop_agent` order `shutdown` uses, but must NOT
/// touch the pool or the live-query dispatcher — those are app-wide and the
/// rest of the running app still needs them. Also pins that it is a no-op
/// when sync was never started, and that a fresh `start_sync_with` after a
/// `stop_sync` works (the toggle can be flipped back on in the same run).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_sync_tears_down_the_lifecycle_without_closing_the_pool() {
    let _env = lifecycle_env_lock().lock().await;

    // A no-op stop_sync before anything ever started must succeed quietly.
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;
    let runtime = tauri_plugin_db::PluginDbRuntime::new(Arc::clone(&db));
    runtime.stop_sync().await.unwrap();

    let (agent_dir, agent) = test_agent().await;
    runtime.start_sync_with(agent).await.unwrap();
    let status = runtime.sync_status().await.unwrap();
    assert!(status.running, "lifecycle started");

    runtime.stop_sync().await.unwrap();

    let status = db.cloudsync_status().await.unwrap();
    assert!(
        !status.running && !status.network_initialized,
        "stop_sync must have run db_cloudsync_stop"
    );

    // The pool must still be open — a runtime opt-out must not disturb the
    // rest of the running app.
    sqlx::query("SELECT 1")
        .fetch_one(db.pool())
        .await
        .expect("pool must stay open after stop_sync");

    // Flipping the setting back on in the same run must work.
    let (agent_dir2, agent2) = test_agent().await;
    runtime.start_sync_with(agent2).await.unwrap();
    let status = runtime.sync_status().await.unwrap();
    assert!(status.running, "restart after stop_sync must succeed");

    runtime.shutdown().await;

    drop(agent_dir);
    drop(agent_dir2);
    drop(db_dir);
}

/// `PluginDbRuntime::start_sync`/`start_sync_with` must be idempotent: a
/// second call while already running is a documented no-op (`if
/// guard.is_some() { return Ok(()); }`), not a silent agent swap. Pins the
/// observable consequence — the second agent never replaces the first — so
/// a future regression (e.g. dropping the `is_some()` guard) shows up as a
/// changed fingerprint rather than passing silently.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_sync_is_idempotent_and_does_not_swap_in_a_second_agent() {
    let _env = lifecycle_env_lock().lock().await;

    let (agent_dir, agent) = test_agent().await;
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;
    let runtime = tauri_plugin_db::PluginDbRuntime::new(Arc::clone(&db));

    runtime.start_sync_with(agent).await.unwrap();
    let first_device = runtime.sync_this_device().await.unwrap();

    let (agent_dir2, agent2) = test_agent().await;
    runtime.start_sync_with(agent2).await.unwrap();
    let second_device = runtime.sync_this_device().await.unwrap();

    assert_eq!(
        first_device, second_device,
        "a second start_sync_with while running must not replace the lifecycle"
    );

    runtime.shutdown().await;

    drop(agent_dir);
    drop(agent_dir2);
    drop(db_dir);
}

/// SYNC-6: `SyncLifecycle::remove_peer` reports whether the peer actually
/// existed — `false` for an unknown fingerprint, `true` (once) for a peer
/// that was paired, and `false` again on a second removal of the same peer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remove_peer_reports_existence_and_is_idempotent() {
    let _env = lifecycle_env_lock().lock().await;

    let (agent_dir, agent) = test_agent().await;
    let db_dir = tempfile::tempdir().unwrap();
    let db = test_db(&db_dir).await;
    let lifecycle = SyncLifecycle::start_with(Arc::clone(&db), agent)
        .await
        .unwrap();

    let unknown_dir = tempfile::tempdir().unwrap();
    let unknown = Identity::load_or_create_in(unknown_dir.path()).unwrap();
    assert!(
        !lifecycle
            .remove_peer(unknown.fingerprint().as_str())
            .unwrap(),
        "removing a peer that was never added reports false"
    );

    let peer_dir = tempfile::tempdir().unwrap();
    let peer = Identity::load_or_create_in(peer_dir.path()).unwrap();
    lifecycle
        .add_peer(peer.fingerprint().as_str(), "Peer")
        .unwrap();

    assert!(
        lifecycle.remove_peer(peer.fingerprint().as_str()).unwrap(),
        "removing a known peer reports true"
    );
    assert!(
        !lifecycle.remove_peer(peer.fingerprint().as_str()).unwrap(),
        "removing the same peer twice is a no-op the second time"
    );

    drop(agent_dir);
    drop(db_dir);
}
