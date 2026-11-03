//! Pairing command implementations for the Tauri plugin layer.
//!
//! Each function here is called from the cfg-gated body of the
//! corresponding `#[tauri::command]` in `commands.rs`. They access the
//! running P2P agent through `PluginDbRuntime::with_agent` and the peer
//! store through `PluginDbRuntime::with_peer_store` (both provided by
//! SyncEngineDb on `runtime.rs`).
//!
//! `with_agent` takes an `AsyncFnOnce(&SyncAgent) -> R`, so every call site
//! below uses an `async |agent| ...` closure (not `|agent| async move {...}`).

use crate::commands::{
    PairingJoinResult, PairingOfferResult, PairingPendingDevice, PairingStatusResult,
};
use crate::runtime::PluginDbRuntime;

/// Errors from pairing operations at the plugin layer.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PairingCommandError {
    #[error("sync agent is not running — enable sync first")]
    AgentNotRunning,
    #[error(transparent)]
    Pairing(#[from] sync_p2p::PairingError),
    #[error(transparent)]
    Agent(#[from] sync_p2p::AgentError),
    #[error("peer store: {0}")]
    Peers(String),
    #[error("{0}")]
    Other(String),
}

/// The local machine's hostname, used as the default device name shown to
/// the peer during pairing. Falls back to "Unknown device" if unavailable.
fn hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "Unknown device".to_string())
}

impl PluginDbRuntime {
    /// Create a pairing offer: generate the link, QR SVG, and expiry.
    pub(crate) async fn pairing_create_offer(
        &self,
    ) -> Result<PairingOfferResult, PairingCommandError> {
        let device_name = hostname();
        let (offer_link, qr_svg, expires_at) = self
            .with_agent(async |agent| {
                let offer = agent
                    .pairing()
                    .create_offer(&agent.node_id(), device_name.clone());
                let link = offer.encode_link();
                let qr = sync_p2p::pairing::qr_svg(&link)?;
                Ok::<_, sync_p2p::PairingError>((link, qr, offer.expires_at))
            })
            .await
            .ok_or(PairingCommandError::AgentNotRunning)??;

        Ok(PairingOfferResult {
            link: offer_link,
            qr_svg,
            expires_at,
        })
    }

    /// Join another device via a pairing link: dial the offerer, exchange
    /// proofs, and return the SAS code for the user to verify.
    pub(crate) async fn pairing_join(
        &self,
        link: String,
    ) -> Result<PairingJoinResult, PairingCommandError> {
        let offer = sync_p2p::PairingOffer::decode(&link)?;

        let outcome = self
            .with_agent(async |agent| agent.pair_with(&offer).await)
            .await
            .ok_or(PairingCommandError::AgentNotRunning)??;

        Ok(PairingJoinResult {
            sas: outcome.sas,
            device_id: outcome.offerer.device_id,
            device_name: outcome.offerer.name,
        })
    }

    /// Get the current pairing status.
    pub(crate) async fn pairing_status(&self) -> Result<PairingStatusResult, PairingCommandError> {
        let status = self
            .with_agent(async |agent| agent.pairing().status())
            .await
            .ok_or(PairingCommandError::AgentNotRunning)?;

        Ok(PairingStatusResult {
            offer_active: status.offer_active,
            offer_expires_at: status.offer_expires_at,
            pending: status
                .pending_confirmations
                .into_iter()
                .map(|p| PairingPendingDevice {
                    device_id: p.device_id,
                    name: p.name,
                    sas: p.sas,
                    expires_at: p.expires_at,
                })
                .collect(),
        })
    }

    /// Confirm or reject a pending pairing. On acceptance, the peer is
    /// added to the allowlist.
    pub(crate) async fn pairing_confirm(
        &self,
        device_id: String,
        accept: bool,
    ) -> Result<(), PairingCommandError> {
        let confirmed = self
            .with_agent(async |agent| agent.pairing().confirm(&device_id, accept))
            .await
            .ok_or(PairingCommandError::AgentNotRunning)??;

        if let Some(peer) = confirmed {
            let node_id = sync_p2p::Fingerprint::parse(&peer.device_id)
                .map_err(|e| PairingCommandError::Other(format!("invalid device id: {e}")))?;

            self.with_peer_store(|store| store.add_peer(node_id, peer.name.clone()))
                .await
                .map_err(|e| PairingCommandError::Peers(e.to_string()))?
                .map_err(|e| PairingCommandError::Peers(e.to_string()))?;
        }

        Ok(())
    }

    /// Revoke a peer: permanently remove from the allowlist and prevent
    /// re-pairing.
    pub(crate) async fn sync_revoke_peer(
        &self,
        fingerprint: String,
    ) -> Result<bool, PairingCommandError> {
        let node_id = sync_p2p::Fingerprint::parse(&fingerprint)
            .map_err(|e| PairingCommandError::Other(format!("invalid fingerprint: {e}")))?;

        let removed = self
            .with_peer_store(|store| store.revoke(&node_id))
            .await
            .map_err(|e| PairingCommandError::Peers(e.to_string()))?
            .map_err(|e| PairingCommandError::Peers(e.to_string()))?;

        Ok(removed)
    }
}
