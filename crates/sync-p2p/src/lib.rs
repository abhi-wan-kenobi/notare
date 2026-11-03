//! P2P sync transport for notare's v0.6 CRDT sync — iroh/QUIC + device
//! identity + a peer allowlist, carrying cr-sqlite change sets (protocol v2).
//!
//! This crate owns:
//! - the **wire protocol** ([`protocol`]) — `Hello`/`Pull`/`Changes`/`Ack`/
//!   `Error` messages, each sealed with the per-pair key from [`crypto`] and
//!   length-prefix framed, on ALPN [`protocol::SYNC_ALPN`];
//! - the **session** ([`session`]) — [`session::SyncSession`] drives one peer,
//!   both directions, against a [`change::ChangeSource`];
//! - the **P2P agent** ([`agent`]) — owns the iroh endpoint, accepts and dials
//!   sessions, enforces the peer allowlist at dial + accept, and runs the
//!   periodic sync tick;
//! - **device identity** ([`identity`]) — a persistent Ed25519 keypair whose
//!   public key is the device id / iroh `EndpointId`;
//! - the **peer allowlist** ([`peers`]) — the local, non-CRDT-synced set of
//!   paired devices that this device will sync with (closes the §12 SSRF
//!   finding).
//!
//! The transport never links SQLite: `plugins/db` implements
//! [`change::ChangeSource`] over cr-sqlite's `crsql_changes` and the transport
//! only moves [`change::WireChange`] rows and per-peer cursors.

pub mod agent;

pub mod change;
pub mod crypto;
pub mod identity;
pub mod pairing;
pub mod peers;
pub mod protocol;
pub mod session;

pub use agent::{
    AgentConfig, AgentError, AgentTransport, DEFAULT_TICK_INTERVAL, P2pAgent, PairingOutcome,
    register_direct_addr,
};
pub use change::{ChangeSource, ChangeSourceError, WireChange, WireCursor, WirePage, WireValue};
pub use identity::{Fingerprint, FingerprintError, Identity, IdentityError};
pub use pairing::{
    AcceptMsg, JoinMsg, PAIRING_ALPN, PairingError, PairingOffer, PairingService, PairingStatus,
    PendingConfirmation, RejectMsg, compute_proof, compute_sas, verify_proof,
};
pub use peers::{Peer, PeerStore, PeersError};
pub use protocol::{ErrorCode, Message, PROTO_VERSION, ProtocolError, Role, SYNC_ALPN};
pub use session::{SessionConfig, SessionError, SessionReport, SyncSession};
