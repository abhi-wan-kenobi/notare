//! The sync round driver: [`P2pSyncDriver`] iterates the allowlist and pulls
//! from every peer.
//!
//! The 0.7 runtime seam (db-core) defines a `SyncDriver` trait with one
//! method, `round()`. This is the P2P implementation of that seam: one
//! round = one [`P2pAgent::sync_with_peer`] client session against every
//! allowlisted peer, sequentially, aggregated.
//!
//! ## Failure policy
//!
//! An unreachable peer is **normal** — a device asleep, offline, or behind
//! NAT is expected on every tick, so a per-peer failure is recorded and the
//! round continues with the remaining peers. A round fails only when
//! *every* peer failed (nothing synced, so callers can see the round was a
//! wash) — or when a peer reported a **fatal** engine error (only `Fatal`
//! stops the loop; `Transient` errors ride to the next tick).

use iroh::PublicKey;

use crate::agent::P2pAgent;
use crate::source::{ChangeSourceError, Cursor, PeerSyncOutcome};

/// Aggregate result of one sync round across all allowlisted peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRoundOutcome {
    /// How many peers were attempted this round.
    pub peers_attempted: usize,
    /// How many peer sessions completed successfully.
    pub peers_succeeded: usize,
    /// How many peer sessions failed (unreachable, refused, or errored).
    pub peers_failed: usize,
    /// Total changes applied across all successful sessions.
    pub applied: usize,
    /// The furthest cursor reached, per peer, of the successful sessions.
    pub cursors: Vec<(PublicKey, Cursor)>,
    /// The per-peer failures, for status reporting. Empty on a clean round.
    pub failures: Vec<(PublicKey, String)>,
}

impl SyncRoundOutcome {
    /// Whether anything actually synced this round.
    pub fn any_synced(&self) -> bool {
        self.peers_succeeded > 0
    }
}

/// Errors from one sync round.
#[derive(Debug, thiserror::Error)]
pub enum SyncDriverError {
    /// Every peer failed this round — the round accomplished nothing.
    #[error("all {0} peers failed this round: {1}")]
    AllPeersFailed(usize, String),
    /// A peer reported a fatal engine error — the loop must stop.
    #[error("fatal sync error from peer {0}: {1}")]
    Fatal(PublicKey, String),
}

/// The P2P implementation of the runtime's sync driver seam: run one
/// client session against every allowlisted peer, sequentially, and
/// aggregate. Constructed over a running [`P2pAgent`]; the background loop
/// calls [`round`] once per tick.
pub struct P2pSyncDriver {
    agent: P2pAgent,
}

impl P2pSyncDriver {
    /// A driver over a running agent. The driver does not own the agent's
    /// lifecycle — the caller starts it and stops it.
    pub fn new(agent: P2pAgent) -> Self {
        Self { agent }
    }

    /// Gracefully stop the underlying agent.
    pub async fn stop(self) {
        self.agent.stop().await;
    }

    /// One sync round: pull from every allowlisted peer.
    ///
    /// Per-peer failures (dial refused by allowlist, offline peer, session
    /// error) are recorded and do not stop the round — an unreachable peer is
    /// normal. A round returns `Err` only when (a) every attempted peer
    /// failed, or (b) a peer surfaced a fatal engine error.
    pub async fn round(&self) -> Result<SyncRoundOutcome, SyncDriverError> {
        let peers = self.agent.peers().list_peers();
        let mut outcome = SyncRoundOutcome {
            peers_attempted: peers.len(),
            peers_succeeded: 0,
            peers_failed: 0,
            applied: 0,
            cursors: Vec::new(),
            failures: Vec::new(),
        };

        for peer in &peers {
            match self.agent.sync_with_peer(&peer.node_id).await {
                Ok(PeerSyncOutcome { applied, cursor }) => {
                    outcome.peers_succeeded += 1;
                    outcome.applied += applied;
                    outcome.cursors.push((peer.node_id, cursor));
                }
                Err(e) => {
                    // A fatal source error (engine misbehaving) stops the
                    // loop; everything else is an ordinary unreachable-peer
                    // tick.
                    if let crate::agent::AgentError::Source(ChangeSourceError::Fatal(message)) = &e
                    {
                        return Err(SyncDriverError::Fatal(peer.node_id, message.clone()));
                    }
                    outcome.peers_failed += 1;
                    outcome.failures.push((peer.node_id, e.to_string()));
                }
            }
        }

        if outcome.peers_succeeded == 0 && !peers.is_empty() {
            let summary = outcome
                .failures
                .iter()
                .map(|(id, msg)| format!("{id}: {msg}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(SyncDriverError::AllPeersFailed(peers.len(), summary));
        }

        Ok(outcome)
    }
}
