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

    /// Start the P2P sync stack (CRRs + agent). Only exists when the `sync`
    /// feature is on AND the target is on the app's sync-platform gate;
    /// elsewhere the db was opened without cr-sqlite and this is never
    /// callable.
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

    /// Start sync with an explicit identity, allowlist and agent config — the
    /// seam tests use, so they never touch the real `<data_dir>/notare/sync/`
    /// identity the app's `start_sync` would load.
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn start_sync_with_config(
        &self,
        identity: sync_p2p::Identity,
        peers: sync_p2p::PeerStore,
        config: sync_p2p::AgentConfig,
    ) -> std::result::Result<(), crate::sync::SyncError> {
        let mut guard = self.sync.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let lifecycle = SyncLifecycle::start_with_config(
            std::sync::Arc::clone(&self.db),
            identity,
            peers,
            config,
        )
        .await?;
        *guard = Some(lifecycle);
        Ok(())
    }

    /// Runtime opt-out: stop the agent while the app keeps running, so
    /// flipping the setting off takes effect immediately. Leaves the pool and
    /// live queries up (app-wide resources) and the CRR tables converted —
    /// their change tracking is local and costs nothing without a peer. A
    /// no-op if sync was never started.
    ///
    /// Holds the sync mutex for the whole call, across `lifecycle.stop()`'s
    /// await: releasing it between `take()` and `stop()` let a concurrent
    /// `start_sync_with_config` see the slot empty and start a second
    /// `SyncLifecycle` — two agents writing cr-sqlite's CRR state on the
    /// same pool at once — while the first was still shutting down.
    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn stop_sync(&self) -> std::result::Result<(), crate::sync::SyncError> {
        let mut guard = self.sync.lock().await;
        if let Some(lifecycle) = guard.take() {
            lifecycle.stop().await;
        }
        Ok(())
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_status(&self) -> crate::sync::SyncStatus {
        crate::sync::SyncStatus {
            engine_loaded: self.db.crsqlite_path().is_some(),
            engine_error: sync_engine_load_error().map(str::to_owned),
            running: self.sync.lock().await.is_some(),
        }
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_trigger(&self) -> std::result::Result<i64, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => lifecycle.trigger().await,
            None => Err(crate::sync::SyncError::NotRunning),
        }
    }

    /// Run `f` against the running agent, holding the sync mutex for the
    /// whole call (so start/stop/shutdown wait behind it). `None` when sync
    /// is not running. Call with an async closure: `with_agent(async |agent|
    /// agent.pair_with(&offer).await)`.
    #[cfg(all(feature = "sync", sync_platform))]
    pub(crate) async fn with_agent<R>(
        &self,
        f: impl AsyncFnOnce(&crate::sync::SyncAgent) -> R,
    ) -> Option<R> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => Some(f(lifecycle.agent()).await),
            None => None,
        }
    }

    /// Run `f` against this device's peer allowlist: the running agent's
    /// store (so a live agent sees the change) or, with sync stopped, the
    /// on-disk store.
    #[cfg(all(feature = "sync", sync_platform))]
    pub(crate) async fn with_peer_store<R>(
        &self,
        f: impl FnOnce(&sync_p2p::PeerStore) -> R,
    ) -> std::result::Result<R, crate::sync::SyncError> {
        let guard = self.sync.lock().await;
        match guard.as_ref() {
            Some(lifecycle) => Ok(f(lifecycle.agent().peers())),
            None => {
                let store = sync_p2p::PeerStore::load_or_create()
                    .map_err(|e| crate::sync::SyncError::Agent(e.into()))?;
                Ok(f(&store))
            }
        }
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_list_peers(&self) -> Vec<sync_p2p::Peer> {
        self.with_peer_store(|store| store.list_peers())
            .await
            .unwrap_or_default()
    }

    #[cfg(all(feature = "sync", sync_platform))]
    pub async fn sync_this_device(&self) -> std::result::Result<String, crate::sync::SyncError> {
        // Must await the mutex, never blocking_lock: the command runs on an
        // async worker, and start_sync/shutdown hold this mutex across awaits.
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

    /// The teardown sequence, exactly once and in order: stop the agent (its
    /// sessions use the pool) → stop the live-query dispatcher → `Db::close`
    /// (finalizes cr-sqlite's per-connection statements, then closes).
    /// Best-effort: a step that fails must not block the rest.
    ///
    /// The runtime stays behind its Arc (Tauri's state map holds a clone for
    /// the whole app lifetime, so it cannot be uniquely unwound here); the
    /// live-query dispatcher is stopped through an explicit `shutdown()` for
    /// exactly that reason.
    pub async fn shutdown(&self) {
        #[cfg(all(feature = "sync", sync_platform))]
        {
            // Bound to a variable, not inlined into the `if let` scrutinee:
            // this must hold the lock across `stop().await` (same reason as
            // `stop_sync`), and relying on `if let`'s temporary-lifetime
            // extension to do that silently is exactly the shape a future
            // edit could change without noticing.
            let mut guard = self.sync.lock().await;
            if let Some(lifecycle) = guard.take() {
                lifecycle.stop().await;
            }
        }

        self.live_query_runtime.shutdown();

        self.db.close().await;
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

/// Whether this build opens the app db with the cr-sqlite engine loaded:
/// only with the `sync` feature on the app's sync-platform gate. Every other
/// configuration opens exactly as before — no extension.
pub const fn crsqlite_available() -> bool {
    cfg!(all(feature = "sync", sync_platform))
}

/// Why the app db was opened WITHOUT the cr-sqlite engine in a build that has
/// it. Set once by the app's open path when the engine load failed and the
/// no-engine retry succeeded; `sync_status` reports it so the Devices page
/// can show the user the actual cause instead of "engine not loaded".
static SYNC_ENGINE_LOAD_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Record the engine load failure. Only the first call sticks.
pub fn set_sync_engine_load_error(reason: String) {
    let _ = SYNC_ENGINE_LOAD_ERROR.set(reason);
}

pub fn sync_engine_load_error() -> Option<&'static str> {
    SYNC_ENGINE_LOAD_ERROR.get().map(String::as_str)
}

pub async fn open_app_db(db_path: Option<&Path>) -> Result<Db> {
    open_app_db_with_crsqlite(db_path, crsqlite_available()).await
}

/// Open the app db with the cr-sqlite engine explicitly on or off.
///
/// Loading the engine converts no table; `start_sync` does that, and only
/// when the user turned sync on. The `false` case is the recovery path: a
/// build whose extension cannot be loaded must still open the database and
/// run as a plain local notes app. If this fails with the engine OFF too,
/// the database itself is broken and that is genuinely fatal. `true` is
/// ignored in builds where [`crsqlite_available`] is false.
pub async fn open_app_db_with_crsqlite(db_path: Option<&Path>, crsqlite: bool) -> Result<Db> {
    let storage = match db_path {
        Some(path) => DbStorage::Local(path),
        None => DbStorage::Memory,
    };
    let options = DbOpenOptions {
        storage,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    };

    #[cfg(all(feature = "sync", sync_platform))]
    let db = if crsqlite {
        Db::open_crsqlite(options).await?
    } else {
        Db::open(options).await?
    };
    #[cfg(not(all(feature = "sync", sync_platform)))]
    let db = {
        let _ = crsqlite;
        Db::open(options).await?
    };

    hypr_db_app::prepare_schema(&db).await?;

    Ok(db)
}
