//! P2P sync transport for notare's v0.6 CRDT sync — iroh/QUIC + device
//! identity + a peer allowlist, building on the S1 convergence spike.
//!
//! This crate owns:
//! - the **broker** ([`broker`]) — the CloudSync control plane + in-memory
//!   object store that collapses the HTTP-S3 3-step upload/apply flow (a peer
//!   serves the CloudSync protocol directly, no S3);
//! - the **P2P agent** ([`agent`]) — the bridge between the synchronous C
//!   network layer and the asynchronous iroh/QUIC transport, which enforces
//!   the peer allowlist at dial + accept;
//! - **device identity** ([`identity`]) — a persistent Ed25519 keypair whose
//!   public key is the device id / iroh `EndpointId`;
//! - the **peer allowlist** ([`peers`]) — the local, non-CRDT-synced set of
//!   paired devices that this device will sync with (closes the §12 SSRF
//!   finding).
//!
//! The actual CloudSync network layer — the two C functions the sqlite-sync
//! core calls — lives in `crates/cloudsync/build/network_p2p.c`, compiled
//! into the loadable `cloudsync.so` under the `from-source` feature. The C
//! layer is deliberately dumb and **local**: it speaks the framed TCP
//! protocol ([`protocol`]) to the in-process [`agent::P2pAgent`] on
//! `127.0.0.1`, and the agent relays each request to the addressed peer over
//! an iroh bi-stream. iroh/QUIC lives entirely in Rust — C never speaks QUIC.
//!
//! See `docs/internal/sync-p2p.md` (§1–§6 for the C contract, §11 for the S1
//! call graph, §12 for the audit, §13 for the SYNC-3 architecture) for the
//! verbatim core call sequence this transport must satisfy.

pub mod agent;
pub mod broker;
pub mod identity;
pub mod peers;
pub mod protocol;

pub use agent::{P2pAgent, register_direct_addr, self_address};
pub use broker::Broker;
pub use identity::{Fingerprint, FingerprintError, Identity, IdentityError};
pub use peers::{Peer, PeerStore, PeersError};
pub use protocol::{Request, Response};
