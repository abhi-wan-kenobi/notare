//! Error kinds for the cr-sqlite engine wrapper.
//!
//! There is no `Auth` kind: cr-sqlite is a local CRDT engine, there is no
//! credential to fail.

/// Whether a failed operation is worth retrying: `Transient` with backoff,
/// `Fatal` not at all.
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
    #[error("crsql_site_id() returned {0} bytes, expected 16")]
    InvalidSiteId(usize),
    #[error("failed to decode a crsql_changes value: {0}")]
    Decode(String),
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            // Pool contention is the same class of "another connection has
            // it, back off and retry" as SQLITE_BUSY/LOCKED.
            Self::Sqlx(sqlx::Error::PoolTimedOut) => ErrorKind::Transient,
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
///
/// `sqlx::Error::Database::code()` returns the *extended* result code
/// (sqlx-sqlite calls `sqlite3_extended_errcode`, not the primary one), so
/// WAL contention routinely shows up as `SQLITE_BUSY_RECOVERY` (261) or
/// `SQLITE_BUSY_SNAPSHOT` (517), not bare `SQLITE_BUSY` (5) — mask to the
/// low byte to recover the primary code before matching.
fn classify_error_code(code: i64) -> ErrorKind {
    match code & 0xff {
        // SQLITE_BUSY (5) / SQLITE_LOCKED (6): another connection holds the
        // lock (WAL allows one writer; a sync apply racing a local write is
        // exactly this). Backoff and retry.
        5 | 6 => ErrorKind::Transient,
        _ => ErrorKind::Fatal,
    }
}
