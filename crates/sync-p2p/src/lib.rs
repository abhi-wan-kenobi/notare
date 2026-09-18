//! P2P sync transport for notare's 0.7 cr-sqlite sync — iroh/QUIC + device
//! identity + peer allowlist + protocol v2.
//!
//! This crate owns:
//! - the **P2P agent** ([`agent`]) — the iroh endpoint that runs pull-only,
//!   symmetric protocol v2 sessions against allowlisted peers, backed by a
//!   [`source::ChangeSource`];
//! - **device identity** ([`identity`]) — a persistent Ed25519 keypair whose
//!   public key is the device id / iroh `EndpointId`;
//! - the **peer allowlist** ([`peers`]) — the local, non-synced set of
//!   paired devices this device will sync with;
//! - the **DB seam** ([`source`]) — the trait between the wire protocol and
//!   the CRDT engine; the concrete `DbChangeSource` lives in the plugin
//!   crate, and this crate's tests drive an in-memory `FakeSource`;
//! - the **round driver** ([`engine`]) — `P2pSyncDriver`, one pull session
//!   against every allowlisted peer per round.
//!
//! Protocol v2 (0.7 engine swap to cr-sqlite) deleted the v1 broker, the
//! localhost TCP listener for the C network layer, the C↔agent bearer token,
//! and the HTTP-shaped request/relay flow: cr-sqlite has no in-process C
//! network layer to bridge. What remains is a direct peer-to-peer pull
//! protocol — whole-frame encrypted with [`crypto`] — plus the unchanged
//! identity/allowlist/crypto/iroh plumbing. See `docs/internal/sync-p2p.md`
//! (§1–§16 describe the retired architecture; §30 the v2 swap).

pub mod agent;
pub mod crypto;
pub mod engine;
pub mod identity;
pub mod peers;
pub mod protocol;
pub mod source;

pub use agent::{AgentError, AgentTransport, P2pAgent, SYNC_ALPN, register_direct_addr};
pub use engine::{P2pSyncDriver, SyncDriverError, SyncRoundOutcome};
pub use identity::{Fingerprint, FingerprintError, Identity, IdentityError};
pub use peers::{Peer, PeerStore, PeersError};
pub use protocol::{DEFAULT_MAX_BYTES, MAX_FRAME_BYTES, SyncMessage};
pub use source::{
    Change, ChangeSource, ChangeSourceError, ChangesPage, Cursor, FakeSource, PeerSyncOutcome,
    SqlValue,
};
