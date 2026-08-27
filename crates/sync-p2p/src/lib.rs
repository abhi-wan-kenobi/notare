//! S1 transport spike — proves a real sqlite-sync (CloudSync) changeset can
//! converge device-to-device over a custom transport with **no** SQLite Cloud,
//! Postgres, or Supabase server.
//!
//! This crate owns the **broker** (a localhost TCP server standing in for the
//! SQLite Cloud + S3 control plane) and the **convergence proof**. The actual
//! CloudSync network layer — the two C functions the sqlite-sync core calls —
//! lives in `crates/cloudsync/build/network_p2p.c` and is compiled into the
//! loadable `cloudsync.so` by `crates/cloudsync/build.rs` under the
//! `from-source` feature. The C layer speaks the framed TCP protocol defined
//! in [`protocol`] to the [`broker::Broker`].
//!
//! ## The collapsed S3 flow
//!
//! The default CloudSync protocol is HTTP-S3-shaped (3-step):
//! `receive(upload)` → parse `{"url":...}` → `send_buffer(url, blob)` (HTTP PUT
//! to S3) → `receive(apply, POST)`. The broker collapses this by serving the
//! `{"url":"mem://..."}` JSON itself and holding the blob in an in-memory
//! object store — a peer serves the CloudSync protocol directly, no S3.
//!
//! See `docs/internal/sync-p2p.md` (and the S1 appendix) for the verbatim core
//! call sequence this transport must satisfy.

pub mod broker;
pub mod protocol;

pub use broker::Broker;
pub use protocol::{Request, Response};
