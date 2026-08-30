//! SYNC-5: the app's sync lifecycle — owning the `P2pAgent` and driving the
//! cloudsync runtime on top of it.
//!
//! ## Why this module exists (and why it is gated)
//!
//! The C `network_p2p.c` layer discovers its agent via two env vars it reads
//! on every network call (`NOTARE_SYNC_AGENT_ADDR`, `NOTARE_SYNC_TOKEN`), and
//! the from-source cloudsync build that layer belongs to is linux/x86_64-only
//! until SYNC-9. So the whole lifecycle lives behind the opt-in `sync` cargo
//! feature **and** `cfg(target_os = "linux")`: a default build of this crate —
//! and of the whole workspace — compiles without an agent, without the env
//! vars, and with `cloudsync_enabled: false`, byte-identical in behavior to
//! pre-SYNC-5.
//!
//! ## Startup order is load-bearing
//!
//! 1. start the `P2pAgent` (identity + allowlist from `<data_dir>/notare/sync/`)
//! 2. publish `NOTARE_SYNC_AGENT_ADDR` + `NOTARE_SYNC_TOKEN` — the single
//!    point of publication; set once, never mutated while the app runs
//! 3. `cloudsync_configure` — `connection_string` is this device's own
//!    `p2p://<fingerprint>` (elected-hub topology: a site's own broker serves
//!    its local pulls), `CloudsyncAuth::None` (iroh's Ed25519 handshake is
//!    the auth), table registry from `hypr_db_app`
//! 4. `cloudsync_start` — its `network_init` runs *inside* here and reads the
//!    env var, so the agent must already be live
//!
//! ## Teardown order is load-bearing (bug class #101)
//!
//! The cloudsync extension holds prepared statements against pool
//! connections; closing the pool first would leave them finalizing against
//! freed handles. So: `cloudsync_stop` (runs `cloudsync_network_cleanup` +
//! `cloudsync_terminate`) → drop live queries → `pool().close()` → stop the
//! agent. [`PluginDbRuntime::shutdown`] performs exactly this sequence.

use hypr_db_core::Db;
use sync_p2p::{P2pAgent, Peer};

/// How often the background sync loop pulls (ms). The SYNC-5 drain fix means
/// one tick fully catches a site up, so this is pacing, not correctness.
const SYNC_INTERVAL_MS: u64 = 2000;

/// A started sync stack: the agent, plus the db it serves. Held by
/// [`crate::runtime::PluginDbRuntime`] while sync is active.
pub struct SyncLifecycle {
    agent: P2pAgent,
    db: std::sync::Arc<Db>,
}

/// Why a sync lifecycle step failed. Sync start is best-effort in the app: a
/// failure leaves the app running with sync disabled, not crashed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("failed to start p2p agent: {0}")]
    Agent(#[from] sync_p2p::AgentError),
    #[error("cloudsync runtime error: {0}")]
    Runtime(#[from] hypr_db_core::CloudsyncRuntimeError),
}

impl SyncLifecycle {
    /// Start sync: agent up → env vars published → cloudsync configured →
    /// cloudsync started. Order per the module docs.
    pub async fn start(db: std::sync::Arc<Db>) -> Result<Self, SyncError> {
        let agent = P2pAgent::start().await?;
        Self::start_with(db, agent).await
    }

    /// Same startup, on an already-running agent — for tests, which supply an
    /// agent rooted at a tempdir identity instead of the real
    /// `<data_dir>/notare/sync/`.
    pub async fn start_with(db: std::sync::Arc<Db>, agent: P2pAgent) -> Result<Self, SyncError> {
        // SAFETY: the C layer getenvs `NOTARE_SYNC_AGENT_ADDR` /
        // `NOTARE_SYNC_TOKEN` per network call, so a `set_var` racing a
        // `getenv` from cloudsync's threads would be UB. The contract that
        // makes this sound: exactly one SyncLifecycle per process. In the app
        // that is structural — one `PluginDbRuntime`, and `start_sync` holds
        // the runtime's sync mutex across this whole call, while the only
        // readers (cloudsync's background loop + network calls) spawn inside
        // the `cloudsync_start` BELOW, after publication. Callers that go
        // through `start_with` directly (tests) must serialize themselves.
        unsafe {
            std::env::set_var("NOTARE_SYNC_AGENT_ADDR", &agent.local_addr);
            std::env::set_var("NOTARE_SYNC_TOKEN", agent.token());
        }

        db.cloudsync_configure(hypr_db_core::CloudsyncRuntimeConfig {
            connection_string: agent.address(),
            auth: hypr_db_core::CloudsyncAuth::None,
            tables: hypr_db_app::cloudsync_table_registry()
                .iter()
                .cloned()
                .collect(),
            sync_interval_ms: SYNC_INTERVAL_MS,
            wait_ms: None,
            max_retries: None,
        })?;

        db.cloudsync_start().await?;

        Ok(Self { agent, db })
    }

    /// The cloudsync runtime status.
    pub async fn status(&self) -> Result<hypr_db_core::CloudsyncStatus, SyncError> {
        Ok(self.db.cloudsync_status().await?)
    }

    /// One immediate sync round (`cloudsync_trigger_sync` — its wrapper drains
    /// until the hub reports nothing pending).
    pub async fn trigger(&self) -> Result<i64, SyncError> {
        Ok(self.db.cloudsync_trigger_sync().await?)
    }

    /// The paired-device allowlist.
    pub fn list_peers(&self) -> Vec<Peer> {
        self.agent.peers().list_peers()
    }

    /// This device's fingerprint string (for display / manual pairing, SYNC-6).
    pub fn this_device(&self) -> String {
        self.agent.node_id().to_z32()
    }

    /// Step 1 of the #101 teardown: finalize extension statements before
    /// anything closes the pool underneath them.
    pub async fn db_cloudsync_stop(&self) -> Result<(), SyncError> {
        Ok(self.db.cloudsync_stop().await?)
    }

    /// Step 4 of the #101 teardown: stop the agent. Split from
    /// [`Self::db_cloudsync_stop`] so [`PluginDbRuntime::shutdown`] can run
    /// steps 2 (drop live queries) and 3 (`pool().close()`) between them.
    pub async fn stop_agent(self) -> Result<(), SyncError> {
        self.agent.stop().await;
        Ok(())
    }
}
