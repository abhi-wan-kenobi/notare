//! Sync lifecycle through `PluginDbRuntime`: two app databases with the
//! cr-sqlite engine, each with its own loopback `P2pAgent`, syncing over real
//! iroh/QUIC on 127.0.0.1 (relay disabled, no external network).

#![cfg(all(feature = "sync", sync_platform))]

use std::sync::Arc;

use sync_p2p::{AgentConfig, AgentTransport, Identity, PeerStore};
use tauri_plugin_db::PluginDbRuntime;
use tauri_plugin_db::sync::SyncError;

struct Device {
    runtime: Arc<PluginDbRuntime>,
    identity_dir: tempfile::TempDir,
    _db_dir: tempfile::TempDir,
}

impl Device {
    async fn open(crsqlite: bool) -> Self {
        let db_dir = tempfile::tempdir().unwrap();
        let db = tauri_plugin_db::open_app_db_with_crsqlite(
            Some(&db_dir.path().join("app.db")),
            crsqlite,
        )
        .await
        .unwrap();
        Self {
            runtime: Arc::new(PluginDbRuntime::new(Arc::new(db))),
            identity_dir: tempfile::tempdir().unwrap(),
            _db_dir: db_dir,
        }
    }

    fn identity(&self) -> Identity {
        Identity::load_or_create_in(self.identity_dir.path()).unwrap()
    }

    async fn start(&self) -> Result<(), SyncError> {
        let identity = self.identity();
        let peers = PeerStore::load_or_create_in(self.identity_dir.path()).unwrap();
        let config = AgentConfig {
            transport: AgentTransport::Loopback,
            tick_interval: None,
            ..AgentConfig::default()
        };
        self.runtime
            .start_sync_with_config(identity, peers, config)
            .await
    }

    async fn fingerprint(&self) -> String {
        self.runtime.sync_this_device().await.unwrap()
    }

    async fn exec(&self, sql: &str) {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(self.runtime.pool())
            .await
            .unwrap();
    }

    async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(self.runtime.pool())
            .await
            .unwrap()
    }
}

async fn paired() -> (Device, Device) {
    let a = Device::open(true).await;
    let b = Device::open(true).await;
    a.start().await.unwrap();
    b.start().await.unwrap();
    a.runtime
        .sync_add_peer(b.fingerprint().await, "B".into())
        .await
        .unwrap();
    b.runtime
        .sync_add_peer(a.fingerprint().await, "A".into())
        .await
        .unwrap();
    (a, b)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_on_one_device_reaches_the_other_through_the_agent() {
    let (a, b) = paired().await;

    a.exec("INSERT INTO sessions (id, title) VALUES ('s1', 'from A')")
        .await;
    a.exec("INSERT INTO tags (id, name) VALUES ('t1', 'work')")
        .await;
    b.exec("INSERT INTO sessions (id, title) VALUES ('s2', 'from B')")
        .await;

    let pulled = b.runtime.sync_trigger().await.unwrap();
    assert!(pulled > 0, "B must pull A's changes");

    for device in [&a, &b] {
        assert_eq!(device.count("SELECT COUNT(*) FROM sessions").await, 2);
        assert_eq!(
            device
                .count("SELECT COUNT(*) FROM tags WHERE id = 't1'")
                .await,
            1
        );
    }
    assert_eq!(
        b.count("SELECT COUNT(*) FROM sync_peer_cursors WHERE db_version > 0")
            .await,
        1,
        "B persisted its cursor into A's stream"
    );

    // Nothing new: the next round transfers nothing and duplicates nothing.
    assert_eq!(b.runtime.sync_trigger().await.unwrap(), 0);
    assert_eq!(a.runtime.sync_trigger().await.unwrap(), 0);
    assert_eq!(b.count("SELECT COUNT(*) FROM sessions").await, 2);

    a.runtime.shutdown().await;
    b.runtime.shutdown().await;
    assert!(a.runtime.pool().is_closed());
    assert!(b.runtime.pool().is_closed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trigger_fails_when_the_only_peer_is_unreachable() {
    let (a, b) = paired().await;
    b.runtime.stop_sync().await.unwrap();

    let error = a.runtime.sync_trigger().await.unwrap_err();
    assert!(matches!(error, SyncError::AllPeersFailed(_)), "{error}");

    a.runtime.shutdown().await;
    b.runtime.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_sync_keeps_the_pool_and_sync_can_restart() {
    let device = Device::open(true).await;
    device.runtime.stop_sync().await.unwrap();
    assert!(matches!(
        device.runtime.sync_trigger().await.unwrap_err(),
        SyncError::NotRunning
    ));

    device.start().await.unwrap();
    let fingerprint = device.fingerprint().await;
    assert!(device.runtime.sync_status().await.running);

    // A second start while running is a no-op, not an agent swap.
    device.start().await.unwrap();
    assert_eq!(device.fingerprint().await, fingerprint);

    device.runtime.stop_sync().await.unwrap();
    let status = device.runtime.sync_status().await;
    assert!(!status.running);
    assert!(status.engine_loaded);
    assert!(!device.runtime.pool().is_closed());
    device
        .exec("INSERT INTO sessions (id, title) VALUES ('after-stop', 'x')")
        .await;

    device.start().await.unwrap();
    assert!(device.runtime.sync_status().await.running);

    device.runtime.shutdown().await;
    assert!(device.runtime.pool().is_closed());
    assert!(!device.runtime.sync_status().await.running);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_sync_refuses_a_database_opened_without_the_engine() {
    let device = Device::open(false).await;
    assert!(!device.runtime.sync_status().await.engine_loaded);

    let error = device.start().await.unwrap_err();
    assert!(matches!(error, SyncError::EngineUnavailable), "{error}");
    assert!(!device.runtime.sync_status().await.running);
    assert_eq!(
        device
            .count("SELECT COUNT(*) FROM sqlite_master WHERE name LIKE '%__crsql_clock'")
            .await,
        0,
        "no table was converted"
    );

    device.runtime.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn add_peer_rejects_garbage_and_self_and_remove_reports_existence() {
    let device = Device::open(true).await;
    device.start().await.unwrap();
    let runtime = &device.runtime;

    assert!(
        runtime
            .sync_add_peer("not-a-real-fingerprint!!".into(), "bad".into())
            .await
            .is_err()
    );
    assert!(
        runtime
            .sync_add_peer(device.fingerprint().await, "myself".into())
            .await
            .is_err(),
        "a device must not add itself"
    );

    let peer_a_dir = tempfile::tempdir().unwrap();
    let peer_a = Identity::load_or_create_in(peer_a_dir.path()).unwrap();
    let peer_b_dir = tempfile::tempdir().unwrap();
    let peer_b = Identity::load_or_create_in(peer_b_dir.path()).unwrap();

    let grouped = peer_a.fingerprint().as_str().to_string();
    assert_eq!(
        runtime
            .sync_add_peer(grouped.clone(), "Peer A".into())
            .await
            .unwrap(),
        grouped
    );
    runtime
        .sync_add_peer(peer_b.fingerprint().compact(), "Peer B".into())
        .await
        .unwrap();
    let peers = runtime.sync_list_peers().await;
    assert_eq!(peers.len(), 2);
    assert!(peers.iter().any(|p| p.node_id == peer_b.id()));

    assert!(runtime.sync_remove_peer(grouped.clone()).await.unwrap());
    assert!(!runtime.sync_remove_peer(grouped).await.unwrap());
    assert_eq!(runtime.sync_list_peers().await.len(), 1);

    runtime.shutdown().await;
}
