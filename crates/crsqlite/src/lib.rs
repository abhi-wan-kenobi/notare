//! cr-sqlite (vlcn-io, MIT, pinned `v0.16.3`) CRDT engine wrapper — the
//! engine that replaces the vendored sqlite-sync (2026-09-07 decision,
//! `docs/LICENSE-NOTE.md` + the 2026-09-07 sync plan).
//!
//! Packaging 1a: the four `sync_platform` targets carry a prebuilt
//! loadable-extension binary under `vendor/<target>/` (MIT LICENSE +
//! SHA256SUMS verified by build.rs), embedded with `include_bytes!` and
//! extracted to `dirs::cache_dir()/notare/crsqlite/<version>/<target>/`. No
//! `OUT_DIR` path, no `resources/` staging, no `tauri.conf.json` resource
//! entry, so the #154 class cannot recur.
//!
//! Modules:
//! - [`apply`] — `SqliteConnectOptions -> (options, extension path)`, the
//!   loader db-core's `Db::open_crsqlite` calls.
//! - [`api`] — `as_crr`, `site_id`, `db_version`, `begin_alter`,
//!   `commit_alter`, `finalize`, `close_pool` (finalize every pooled
//!   connection, then close), `is_crr` (`<table>__crsql_clock` in
//!   `sqlite_master`), `load_probe` (`SELECT crsql_site_id()`; no
//!   `crsql_version()` exists at v0.16.3).
//! - [`changes`] — `SqlValue`, `Change`, `Cursor {db_version, seq}`,
//!   `pull_changes(after, exclude_site, limit) -> ChangesPage
//!   {changes, next, more}` with tuple paging `(db_version, seq) > (?, ?)`,
//!   `apply_changes(conn, &[Change])` with no transaction of its own.
//! - [`error`] — `Transient`/`Fatal` kinds, so callers can tell a retryable
//!   lock-contention failure from one that needs intervention.
//!
//! The engine tests (`tests/engine_*.rs`) load the **vendored** binary via
//! [`apply`]/[`bundled_extension_path`] unconditionally on every supported
//! target — this is what the `crsqlite_gate_{linux,macos,windows}` CI jobs
//! run. The Phase 0 spike harness lives in `tests/common/spike.rs`; its
//! env-gated tests (`CRSQLITE_SPIKE_EXTENSION`) drive an arbitrary local
//! build of the engine.

#![deny(unsafe_code)]

mod api;
mod bundle;
mod changes;
mod error;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use api::{
    as_crr, begin_alter, close_pool, commit_alter, db_version, finalize, is_crr, load_probe,
    site_id,
};
pub use bundle::bundled_extension_path;
pub use changes::{Change, ChangesPage, Cursor, SqlValue, apply_changes, pull_changes};
pub use error::{Error, ErrorKind};

/// Pinned upstream release the vendored binaries are from
/// (vlcn-io/cr-sqlite `v0.16.3`, MIT). Bump = re-vendor all four targets +
/// SHA256SUMS + this const together.
pub const CRSQLITE_VERSION: &str = "0.16.3";

/// Load the vendored cr-sqlite engine onto every connection built from
/// `options`, returning the options and the extracted extension path.
///
/// Extracts the embedded binary for the current target to the cache dir
/// (see [`bundled_extension_path`]) and registers it as a loadable
/// extension.
pub fn apply(options: SqliteConnectOptions) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    let extension_path = bundled_extension_path()?;
    let extension_path_str = extension_path
        .clone()
        .into_os_string()
        .into_string()
        .map_err(|raw| Error::NonUtf8ExtensionPath(raw.to_string_lossy().into_owned()))?;

    // SAFETY: the extension is the vendored vlcn-io v0.16.3 prebuilt
    // release binary — a first-party file this crate ships, verified
    // against vendor/SHA256SUMS by build.rs. The crate-level `#![deny(unsafe_code)]`
    // cannot be violated from this function without the allow below being
    // deleted in review, which is the point of keeping it explicit.
    #[allow(unsafe_code)]
    let options = unsafe { options.extension(extension_path_str) };

    Ok((options, extension_path))
}
