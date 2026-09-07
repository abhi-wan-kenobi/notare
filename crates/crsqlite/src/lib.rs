//! Phase 0 STRICT spike for the cr-sqlite (vlcn-io) CRDT engine swap.
//!
//! 0.7 engine decision (2026-09-07, `docs/LICENSE-NOTE.md` + the 2026-09-07
//! sync plan): the vendored sqlite-sync network-layer carve-out forbids the
//! P2P seam Notare is built on, so the CRDT engine becomes cr-sqlite
//! (vlcn-io, MIT, pinned `v0.16.3`). Before any rewiring, this crate answers
//! the one gating question — **can cr-sqlite carry the app's STRICT schema
//! end-to-end without textualising values?** — plus the two operational
//! unknowns the toolchain surfaced (sqlx's `sqlite3_close` Drop panic under
//! pooling, and whether remote applies fire the update hook for free).
//!
//! Nothing here is throwaway scaffolding: `tests/strict_roundtrip.rs` is the
//! permanent CI gate (`crsqlite_gate_*` jobs land in PR C-B once the vendored
//! prebuilt binaries make it runnable on every runner without the env var).
//!
//! The engine itself is **not** vendored in this crate yet (that is PR C-B's
//! packaging work). Tests load a prebuilt `crsqlite.so`/`.dylib`/`.dll` from
//! the vlcn-io `v0.16.3` release via `CRSQLITE_SPIKE_EXTENSION` (an absolute
//! path); when the variable is unset every test prints "skipped" and returns
//! — the same gating pattern as `SQLITECLOUD_URL` in
//! `crates/db-core/tests/e2e.rs`. Nothing on the default `cargo test` path
//! needs the extension.
//!
//! Dependency posture: db-app/db-migrate/db-core/db-change are *dev-facing*
//! here (used from `tests/`, exercised through this crate's [dev-dependencies]
//! which, for a crate with no shipped lib consumers yet, is where they
//! belong until PR C-B promotes the crate to the real engine wrapper).
//! `sqlx` features mirror `crates/cloudsync` (`sqlite-unbundled`, plus the
//! facade `sqlite` feature which turns on `sqlite-load-extension` — the
//! `SqliteConnectOptions::extension()` API the dynamic load style uses,
//! already proven on this stack by `crates/cloudsync/src/lib.rs`).

#![deny(unsafe_code)]

/// Pinned upstream release the spike (and later the vendored binaries) targets.
pub const CRSQLITE_VERSION: &str = "0.16.3";

pub mod spike;