use std::path::Path;

use hypr_db_core::{Db, DbOpenOptions, DbStorage};
use hypr_db_execute::{DbExecutor, ProxyQueryMethod, ProxyQueryResult};
use hypr_db_reactive::{LiveQueryRuntime, QueryEventSink, SubscriptionRegistration};
use tauri::ipc::Channel;

#[cfg(all(feature = "sync", sync_platform))]
use crate::sync::SyncLifecycle;
use crate::{QueryEvent, Result, TransactionStatement};

#[derive(Clone)]
pub struct QueryEventChannel(Channel<QueryEvent>);

impl QueryEventChannel {
    pub fn new(channel: Channel<QueryEvent>) -> Self {
        Self(channel)
    }
}

impl QueryEventSink for QueryEventChannel {
    fn send_result(&self, rows: Vec<serde_json::Value>) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Result(rows))
            .map_err(|error| error.to_string())
    }

    fn send_error(&self, error: String) -> std::result::Result<(), String> {
        self.0
            .send(QueryEvent::Error(error))
            .map_err(|error| error.to_string())
    }
}

pub struct PluginDbRuntime {
    db: std::sync::Arc<Db>,
    schema_ready: tokio::sync::OnceCell<()>,
    executor: DbExecutor,
    live_query_runtime: LiveQueryRuntime<QueryEventChannel>,
    #[cfg(all(feature = "sync", sync_platform))]
    sync: tokio::sync::Mutex<Option<SyncLifecycle>>,
}

impl PluginDbRuntime {
    pub fn new(db: std::sync::Arc<Db>) -> Self {
        Self {
            db: std::sync::Arc::clone(&db),
            schema_ready: tokio::sync::OnceCell::new(),
            executor: DbExecutor::new(std::sync::Arc::clone(&db)),
            live_query_runtime: LiveQueryRuntime::new(std::sync::Arc::clone(&db)),
            #[cfg(all(feature = "sync", sync_platform))]
            sync: tokio::sync::Mutex::new(None),
        }
    }

    pub fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    async fn ensure_app_schema(&self) -> Result<()> {
        self.schema_ready
            .get_or_try_init(|| async { hypr_db_app::prepare_schema(self.db.as_ref()).await })
            .await?;
        Ok(())
    }

    pub async fn execute(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_app_schema().await?;
        Ok(self.executor.execute(sql, params).await?)
    }

    pub async fn execute_transaction(
        &self,
        statements: Vec<TransactionStatement>,
    ) -> Result<Vec<u64>> {
        self.ensure_app_schema().await?;
        let mut transaction = self.db.pool().begin_with("BEGIN IMMEDIATE").await?;
        let mut rows_affected = Vec::with_capacity(statements.len());

        for (statement_index, statement) in statements.into_iter().enumerate() {
            let result = bind_params(
                sqlx::query(sqlx::AssertSqlSafe(statement.sql.as_str())),
                &statement.params,
            )
            .execute(&mut *transaction)
            .await?;
            let actual = result.rows_affected();
            if let Some(expected) = statement.expected_rows_affected
                && actual != expected
            {
                return Err(crate::Error::UnexpectedRowsAffected {
                    statement_index,
                    expected,
                    actual,
                });
            }
            rows_affected.push(actual);
        }

        transaction.commit().await?;
        Ok(rows_affected)
    }

    pub async fn execute_proxy(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        method: ProxyQueryMethod,
    ) -> Result<ProxyQueryResult> {
        self.ensure_app_schema().await?;
        Ok(self.executor.execute_proxy(sql, params, method).await?)
    }

    pub async fn subscribe(
        &self,
        sql: String,
        params: Vec<serde_json::Value>,
        sink: QueryEventChannel,
    ) -> Result<SubscriptionRegistration> {
        self.ensure_app_schema().await?;
        Ok(self.live_query_runtime.subscribe(sql, params, sink).await?)
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> hypr_db_reactive::Result<()> {
        self.live_query_runtime.unsubscribe(subscription_id).await
    }

    /// SYNC-5: start the P2P sync stack (agent + cloudsync). Only exists when
    /// the `sync` feature is on AND the target is on the app's sync-platform
    /// gate (§22); elsewhere the db was opened with `cloudsync_enabled:
    /// false` and this is never callable.
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn start_sync(&self) -> std::result::Result<(), crate::sync::SyncError> {
        let mut guard = self.sync.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let lifecycle = SyncLifecycle::start(std::sync::Arc::clone(&self.db)).await?;
        *guard = Some(lifecycle);
        Ok(())
    }

    /// Install a sync lifecycle on an already-running agent — the seam the
    /// lifecycle test uses, so it never touches the real
    /// `<data_dir>/notare/sync/` identity the app's `start_sync` would load.
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn start_sync_with(
        &self,
        agent: sync_p2p::P2pAgent,
    ) -> std::result::Result<(), crate::sync::SyncError> {
        let mut guard = self.sync.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let lifecycle = SyncLifecycle::start_with(std::sync::Arc::clone(&self.db), agent).await?;
        *guard = Some(lifecycle);
        Ok(())
    }

    /// Runtime opt-out: stop the sync lifecycle while the app keeps running,
    /// so flipping the setting off takes effect immediately (SYNC's runtime
    /// opt-in). Reuses `SyncLifecycle`'s own teardown steps 1 and 4 (the
    /// `db_cloudsync_stop` → `stop_agent` half of the #101 sequence in
    /// [`Self::shutdown`]) but — unlike `shutdown` — leaves the live-query
    /// dispatcher and the pool open, since those are app-wide resources the
    /// rest of the running app still needs. A no-op if sync was never
    /// started.
    ///
    /// `db_cloudsync_stop` failing must never skip `stop_agent`: the guard
    /// above already took the lifecycle out of `self.sync`, so a bailout
    /// here (via `?`) would leak a P2P agent that is still live — still
    /// bound to the network and, in `Discovered` mode, still publishing
    /// itself — while the rest of the app believes sync is off. So this
    /// warns and continues through `cloudsync_stop`, same as `shutdown`, and
    /// only propagates `stop_agent`'s own error (the step that actually
    /// matters for "is this device still reachable").
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn stop_sync(&self) -> std::result::Result<(), crate::sync::SyncError> {
        let lifecycle = {
            let mut guard = self.sync.lock().await;
            guard.take()
        };

        let Some(lifecycle) = lifecycle else {
            return Ok(());
        };

        if let Err(error) = lifecycle.db_cloudsync_stop().await {
            tracing::warn!("stop_sync: db_cloudsync_stop failed: {error}");
        }

        lifecycle.stop_agent().await
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_status(
        &self,
    ) -> std::result::Result<hypr_db_core::CloudsyncStatus, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => Ok(lifecycle.status().await?),
            None => Ok(self.db.cloudsync_status().await?),
        }
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_trigger(&self) -> std::result::Result<i64, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => Ok(lifecycle.trigger().await?),
            None => Ok(self.db.cloudsync_trigger_sync().await?),
        }
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_list_peers(&self) -> Vec<sync_p2p::Peer> {
        // The agent's PeerStore is a cheap Arc clone over the same on-disk
        // allowlist; reading it does not need the lifecycle up.
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => lifecycle.list_peers(),
            None => sync_p2p::PeerStore::load_or_create()
                .map(|peers| peers.list_peers())
                .unwrap_or_default(),
        }
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_this_device(&self) -> std::result::Result<String, crate::sync::SyncError> {
        // Must await the mutex, never blocking_lock: the command runs on an
        // async worker, and start_sync/shutdown hold this mutex across awaits
        // (network init) — a blocking acquire would stall the executor thread,
        // or deadlock it outright on a current_thread runtime.
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => Ok(lifecycle.this_device()),
            None => sync_p2p::Identity::load_or_create()
                .map(|identity| identity.fingerprint().as_str().to_string())
                .map_err(|e| crate::sync::SyncError::Agent(e.into())),
        }
    }

    /// Add a peer to this device's allowlist. Works whether the sync lifecycle
    /// is currently running or not: the allowlist is a local file, so the
    /// PeerStore can be opened directly. This keeps the SYNC-5 "sync is
    /// best-effort" behavior (a not-started agent does not block pairing).
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_add_peer(
        &self,
        fingerprint: String,
        label: String,
    ) -> std::result::Result<String, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => lifecycle.add_peer(&fingerprint, &label),
            None => {
                let store = sync_p2p::PeerStore::load_or_create()
                    .map_err(|e| crate::sync::SyncError::Agent(e.into()))?;
                let node_id = sync_p2p::Fingerprint::parse(&fingerprint).map_err(|e| {
                    crate::sync::SyncError::Peer(format!("invalid fingerprint: {e}"))
                })?;
                let identity = sync_p2p::Identity::load_or_create()
                    .map_err(|e| crate::sync::SyncError::Agent(e.into()))?;
                if node_id == identity.id() {
                    return Err(crate::sync::SyncError::Peer(
                        "cannot add this device as its own peer".to_string(),
                    ));
                }
                store.add_peer(node_id, &label).map_err(|e| {
                    crate::sync::SyncError::Peer(format!("failed to add peer: {e}"))
                })?;
                Ok(sync_p2p::Fingerprint::from_pubkey(&node_id)
                    .as_str()
                    .to_string())
            }
        }
    }

    /// Remove a peer from this device's allowlist. Like `sync_add_peer`, this
    /// works whether the lifecycle is running or not because the allowlist is
    /// a local file.
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_remove_peer(
        &self,
        fingerprint: String,
    ) -> std::result::Result<bool, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => lifecycle.remove_peer(&fingerprint),
            None => {
                let store = sync_p2p::PeerStore::load_or_create()
                    .map_err(|e| crate::sync::SyncError::Agent(e.into()))?;
                let node_id = sync_p2p::Fingerprint::parse(&fingerprint).map_err(|e| {
                    crate::sync::SyncError::Peer(format!("invalid fingerprint: {e}"))
                })?;
                store.remove_peer(&node_id).map_err(|e| {
                    crate::sync::SyncError::Peer(format!("failed to remove peer: {e}"))
                })
            }
        }
    }

    /// The #101 teardown sequence, exactly once and in order:
    /// `cloudsync_stop` → stop live-query dispatcher → `pool().close()` →
    /// stop agent. Best-effort: a step that fails must not block the rest —
    /// the remaining steps are what actually release resources the OS would
    /// otherwise reclaim at process death.
    ///
    /// The runtime stays behind its Arc (Tauri's state map holds a clone for
    /// the whole app lifetime, so it cannot be uniquely unwound here); the
    /// live-query dispatcher is stopped through an explicit `shutdown()` for
    /// exactly that reason.
    pub async fn shutdown(&self) {
        #[cfg(all(feature = "sync", sync_platform))]
        let lifecycle = {
            let mut guard = self.sync.lock().await;
            guard.take()
        };

        #[cfg(all(feature = "sync", sync_platform))]
        if let Some(lifecycle) = lifecycle.as_ref() {
            // (1) cloudsync_stop first: the extension finalizes prepared
            // statements against live pool connections here.
            if let Err(error) = lifecycle.db_cloudsync_stop().await {
                tracing::warn!("sync shutdown: cloudsync_stop failed: {error}");
            }
        }

        // (2) live queries hold pool connections open and their dispatcher
        // holds a Db handle — stop it before closing the pool. Runs on the
        // default path too: closing a pool under a live dispatcher is what
        // #101 is about, sync or not.
        self.live_query_runtime.shutdown();

        // (3) the pool itself.
        self.db.pool().close().await;

        // (4) the agent last: the C layer is gone, nothing relays through it
        // anymore.
        #[cfg(all(feature = "sync", sync_platform))]
        if let Some(lifecycle) = lifecycle {
            if let Err(error) = lifecycle.stop_agent().await {
                tracing::warn!("sync shutdown: stop agent failed: {error}");
            }
        }
    }
}

fn bind_params<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    params: &[serde_json::Value],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    query
}

/// Whether this build would attempt to load the cloudsync extension.
///
/// SYNC-5: cloudsync is only loadable in the from-source build, which the
/// `sync` feature turns on, on the app's sync-platform gate (§22). Every
/// other configuration must keep opening exactly as before — no extension,
/// no env vars. Must match the `sync_platform` cfg exactly (previously
/// checked only `target_os = "linux"`, without the arch/OS set the dependency
/// graph actually gates on — a latent `cloudsync_enabled: true` claim on a
/// target where nothing loads).
pub const fn cloudsync_available() -> bool {
    cfg!(all(feature = "sync", sync_platform))
}

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db> {
    open_app_db_with_cloudsync(db_path, cloudsync_available()).await
}

/// Open the app db with cloudsync explicitly on or off.
///
/// The `false` case is the recovery path: a cloudsync build whose extension
/// cannot be loaded must still be able to open the database and run as a
/// plain local notes app. Reserving that decision for the caller keeps the
/// distinction honest — if this fails with cloudsync OFF too, the database
/// itself is broken and that is genuinely fatal.
pub async fn open_app_db_with_cloudsync(
    db_path: Option<&Path>,
    cloudsync_enabled: bool,
) -> Result<Db> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };

    let db = Db::open(DbOpenOptions {
        storage,
        cloudsync_enabled,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await?;

    hypr_db_app::prepare_schema(&db).await?;

    Ok(db)
}
