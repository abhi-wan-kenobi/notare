//! Error kinds for the cr-sqlite engine wrapper.
//!
//! `Transient`/`Fatal` are kept from `hypr_cloudsync`'s error module (with
//! `Auth` dropped — cr-sqlite is a local CRDT engine, there is no credential
//! to fail) so the runtime retry classifier in db-core
//! (`cloudsync_background_loop`'s `Transient`-vs-`Fatal` branch) survives
//! the engine swap unchanged.

/// Whether a failed operation is worth retrying.
///
/// The sync runtime loop retries `Transient` errors with backoff and stops
/// the loop on `Fatal` ones (only `Fatal` engine errors halt the loop per the
/// 2026-09-07 plan).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Lock contention (SQLITE_BUSY/SQLITE_LOCKED) or a busy database —
    /// retry with backoff.
    Transient,
    /// Schema error, protocol mismatch, corrupted page — retrying
    /// identically cannot fix it; needs intervention.
    Fatal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no cache directory is available for the bundled crsqlite extension")]
    MissingCacheDir,
    #[error(
        "the bundled cr-sqlite engine is not vendored for this target; vendored targets: \
         linux-x86_64, darwin-aarch64, darwin-x86_64, win-x86_64"
    )]
    UnsupportedBundledCrsqlite,
    #[error("the bundled crsqlite extension path is not valid UTF-8: {0}")]
    NonUtf8ExtensionPath(String),
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Sqlx(sqlx_err) => {
                if let Some(code) = extract_error_code(sqlx_err) {
                    classify_error_code(code)
                } else {
                    ErrorKind::Fatal
                }
            }
            // Packaging problems are never transient — retrying cannot make
            // a missing cache dir appear or vendor a target.
            _ => ErrorKind::Fatal,
        }
    }
}

fn extract_error_code(err: &sqlx::Error) -> Option<i64> {
    match err {
        sqlx::Error::Database(db_err) => db_err.code().and_then(|c| c.parse::<i64>().ok()),
        _ => None,
    }
}

/// SQLite primary error codes (native, < 10_000 — cr-sqlite raises no
/// engine-specific extension codes on the paths this crate drives).
fn classify_error_code(code: i64) -> ErrorKind {
    match code {
        // SQLITE_BUSY (5) / SQLITE_LOCKED (6): another connection holds the
        // lock (WAL allows one writer; a sync apply racing a local write is
        // exactly this). Backoff and retry.
        5 | 6 => ErrorKind::Transient,
        _ => ErrorKind::Fatal,
    }
}
