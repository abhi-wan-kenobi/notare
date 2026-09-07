//! cr-sqlite (vlcn-io, MIT, pinned `v0.16.3`) CRDT engine wrapper — the
//! engine that replaces the vendored sqlite-sync (2026-09-07 decision,
//! `docs/LICENSE-NOTE.md` + the 2026-09-07 sync plan).
//!
//! Packaging 1a: the four `sync_platform` targets carry a prebuilt
//! loadable-extension binary under `vendor/<target>/` (MIT LICENSE +
//! SHA256SUMS verified by build.rs), embedded with `include_bytes!` and
//! extracted to `dirs::cache_dir()/notare/crsqlite/<version>/<target>/` —
//! ported from `crates/cloudsync/src/bundle.rs`. No `OUT_DIR` path, no
//! `resources/` staging, no `tauri.conf.json` resource entry, so the #154
//! class cannot recur.
//!
//! Modules:
//! - [`apply`] — the same `SqliteConnectOptions -> (options, path)` signature
//!   as `hypr_cloudsync::apply`, so db-core's call sites change only the
//!   crate path.
//! - [`api`] — `as_crr`, `site_id`, `db_version`, `begin_alter`,
//!   `commit_alter`, `finalize`, `is_crr` (`<table>__crsql_clock` in
//!   `sqlite_master`), `load_probe` (`SELECT crsql_site_id()`; no
//!   `crsql_version()` exists at v0.16.3).
//! - [`changes`] — `SqlValue`, `Change`, `Cursor {db_version, seq}`,
//!   `pull_changes(after, exclude_site, max_bytes) -> ChangesPage
//!   {changes, next, more}` with tuple paging `(db_version, seq) > (?, ?)`,
//!   `apply_changes(conn, &[Change])` with no transaction of its own.
//! - [`error`] — `Transient`/`Fatal` kinds kept (so the runtime retry
//!   classifier survives); `Auth` dropped (no credential to fail).
//! - [`spike`] — the Phase 0 harness; its env-gated tests
//!   (`CRSQLITE_SPIKE_EXTENSION`) remain for driving an arbitrary local
//!   build of the engine.
//!
//! The engine tests (`tests/engine_*.rs`) load the **vendored** binary via
//! [`apply`]/`bundled_extension_path`] unconditionally on every supported
//! target — this is what the `crsqlite_gate_{linux,macos,windows}` CI jobs
//! run.

#![deny(unsafe_code)]

mod api;
mod bundle;
mod changes;
mod error;
pub mod spike;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use api::{
    as_crr, begin_alter, commit_alter, db_version, finalize, is_crr, load_probe, site_id,
};
pub use bundle::bundled_extension_path;
pub use changes::{apply_changes, pull_changes, Change, ChangesPage, Cursor, SqlValue};
pub use error::{Error, ErrorKind};

/// Pinned upstream release the vendored binaries are from
/// (vlcn-io/cr-sqlite `v0.16.3`, MIT). Bump = re-vendor all four targets +
/// SHA256SUMS + this const together.
pub const CRSQLITE_VERSION: &str = "0.16.3";

/// Load the vendored cr-sqlite engine onto every connection built from
/// `options` — the same `(SqliteConnectOptions) -> (options, path)`
/// signature as `hypr_cloudsync::apply`, so db-core's call sites change
/// only the crate path.
///
/// Extracts the embedded binary for the current target to the cache dir
/// (see [`bundled_extension_path`]) and registers it as a loadable
/// extension.
pub fn apply(options: SqliteConnectOptions) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    let extension_path = bundled_extension_path()?;

    // SAFETY: the extension is the vendored vlcn-io v0.16.3 prebuilt
    // release binary — a first-party file this crate ships, verified
    // against vendor/SHA256SUMS by build.rs, the same trust domain as the
    // cloudsync prebuilt this loading style was already proven with
    // (`crates/cloudsync/src/lib.rs`). The crate-level `#![deny(unsafe_code)]`
    // cannot be violated from this function without the allow below being
    // deleted in review, which is the point of keeping it explicit.
    #[allow(unsafe_code)]
    let options = unsafe { options.extension(extension_path.to_string_lossy().into_owned()) };

    Ok((options, extension_path))
}