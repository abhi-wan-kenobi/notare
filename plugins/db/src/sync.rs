//! The app's P2P sync lifecycle: cr-sqlite CRRs plus the protocol-v2
//! `P2pAgent` serving and pulling them.
//!
//! ## Why this module is gated
//!
//! Everything here exists only with the opt-in `sync` cargo feature on the
//! app's `sync_platform` gate (build-support/sync_app_gate.rs). A default
//! build opens the database without cr-sqlite and has no agent.
//!
//! ## Startup order
//!
//! 1. `crsql_as_crr` on [`hypr_db_app::SYNCED_TABLES`] — only here, so a
//!    device that never turns sync on never converts a table.
//! 2. start the `P2pAgent` (identity + allowlist from `<data_dir>/notare/sync/`)
//!    over a [`CrsqliteChangeSource`]; it runs its own accept loop and
//!    periodic all-peers tick.
//!
//! ## Teardown order
//!
//! The agent's sessions read and write the pool, so it stops first; then the
//! live-query dispatcher; then `Db::close`, which finalizes every cr-sqlite
//! connection before `sqlite3_close` (an unfinalized close panics sqlx's
//! worker threads). [`crate::runtime::PluginDbRuntime::shutdown`] performs
//! exactly this sequence.

use std::sync::Arc;

use hypr_db_core::Db;
use sync_p2p::{P2pAgent, Peer};

use crate::change_source::{CrsqliteChangeSource, enable_crr_tables};

pub type SyncAgent = P2pAgent<CrsqliteChangeSource>;

/// What `sync_status` reports: whether this database has the engine loaded,
/// and whether the agent is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncStatus {
    pub engine_loaded: bool,
    pub running: bool,
}

/// A started sync stack. Held by [`crate::runtime::PluginDbRuntime`] while
/// sync is active.
pub struct SyncLifecycle {
    agent: SyncAgent,
}

/// Why a sync lifecycle step failed. Sync start is best-effort in the app: a
/// failure leaves the app running with sync disabled, not crashed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("p2p agent error: {0}")]
    Agent(#[from] sync_p2p::AgentError),
    #[error("the database was opened without the cr-sqlite engine")]
    EngineUnavailable,
    #[error("sync is not running")]
    NotRunning,
    #[error("sync engine error: {0}")]
    Engine(#[from] hypr_crsqlite::Error),
    #[error("sync failed with every peer: {0}")]
    AllPeersFailed(String),
    #[error("peer operation failed: {0}")]
    Peer(String),
}

impl SyncLifecycle {
    /// Start sync on the persisted identity + allowlist with real discovery.
    pub async fn start(db: Arc<Db>) -> Result<Self, SyncError> {
        let source = Self::prepare(&db).await?;
        let agent = P2pAgent::start(source).await?;
        Ok(Self { agent })
    }

    /// Same startup, with the caller choosing identity, allowlist and agent
    /// configuration — tests use loopback transport and tempdir identities.
    /// On `AgentTransport::Loopback` the agent's addresses are published to
    /// the process-local registry, the only way loopback peers resolve it.
    pub async fn start_with_config(
        db: Arc<Db>,
        identity: sync_p2p::Identity,
        peers: sync_p2p::PeerStore,
        config: sync_p2p::AgentConfig,
    ) -> Result<Self, SyncError> {
        let loopback = config.transport == sync_p2p::AgentTransport::Loopback;
        let source = Self::prepare(&db).await?;
        let agent = P2pAgent::start_with_config(identity, peers, source, config).await?;
        if loopback {
            sync_p2p::register_direct_addr(agent.node_id(), agent.direct_addresses()).await;
        }
        Ok(Self { agent })
    }

    async fn prepare(db: &Arc<Db>) -> Result<Arc<CrsqliteChangeSource>, SyncError> {
        if db.crsqlite_path().is_none() {
            return Err(SyncError::EngineUnavailable);
        }
        enable_crr_tables(db).await?;
        Ok(Arc::new(CrsqliteChangeSource::new(Arc::clone(db))))
    }

    pub fn agent(&self) -> &SyncAgent {
        &self.agent
    }

    /// One immediate round with every allowlisted peer. Returns the number of
    /// changes pulled; fails only if there were peers and every one failed.
    pub async fn trigger(&self) -> Result<i64, SyncError> {
        let results = self.agent.sync_all().await;
        let mut pulled = 0usize;
        let mut failures = Vec::new();
        for (peer, result) in &results {
            match result {
                Ok(report) => pulled += report.changes_pulled,
                Err(error) => {
                    tracing::warn!("sync with {} failed: {error}", peer.to_z32());
                    failures.push(format!("{}: {error}", peer.to_z32()));
                }
            }
        }
        if !results.is_empty() && failures.len() == results.len() {
            return Err(SyncError::AllPeersFailed(failures.join("; ")));
        }
        Ok(i64::try_from(pulled).unwrap_or(i64::MAX))
    }

    /// The paired-device allowlist.
    pub fn list_peers(&self) -> Vec<Peer> {
        self.agent.peers().list_peers()
    }

    /// This device's fingerprint string (grouped, dashed display form) —
    /// matches the not-started fallback in
    /// `PluginDbRuntime::sync_this_device`.
    pub fn this_device(&self) -> String {
        sync_p2p::Fingerprint::from_pubkey(&self.agent.node_id())
            .as_str()
            .to_string()
    }

    /// Add a peer to this device's allowlist. Returns the peer's fingerprint.
    pub fn add_peer(&self, fingerprint: &str, label: &str) -> Result<String, SyncError> {
        let node_id = sync_p2p::Fingerprint::parse(fingerprint)
            .map_err(|e| SyncError::Peer(format!("invalid fingerprint: {e}")))?;

        if node_id == self.agent.node_id() {
            return Err(SyncError::Peer(
                "cannot add this device as its own peer".to_string(),
            ));
        }

        self.agent
            .peers()
            .add_peer(node_id, label)
            .map_err(|e| SyncError::Peer(format!("failed to add peer: {e}")))?;

        Ok(sync_p2p::Fingerprint::from_pubkey(&node_id)
            .as_str()
            .to_string())
    }

    /// Remove a peer from this device's allowlist. Returns whether it existed.
    pub fn remove_peer(&self, fingerprint: &str) -> Result<bool, SyncError> {
        let node_id = sync_p2p::Fingerprint::parse(fingerprint)
            .map_err(|e| SyncError::Peer(format!("invalid fingerprint: {e}")))?;

        self.agent
            .peers()
            .remove_peer(&node_id)
            .map_err(|e| SyncError::Peer(format!("failed to remove peer: {e}")))
    }

    /// Stop the agent: no new sessions, endpoint closed. Must run before the
    /// pool closes.
    pub async fn stop(self) {
        self.agent.stop().await;
    }
}
