const SUPPORTED_CLOUDSYNC_TARGETS: &str = concat!(
    "macos/{aarch64,x86_64}, ",
    "ios (via bundled CloudSync.xcframework), ",
    "android/{arm64-v8a,armeabi-v7a,x86_64}, ",
    "linux/{gnu,musl}/{aarch64,x86_64}, ",
    "windows/x86_64"
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Network timeout, connection drop, server pressure — retry with backoff.
    Transient,
    /// Credentials expired or invalid — stop syncing, surface to UI.
    Auth,
    /// TLS, bad URL, protocol mismatch, schema error — needs intervention.
    Fatal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no cache directory is available for the bundled cloudsync extension")]
    MissingCacheDir,
    #[error(
        "the bundled cloudsync extension is not available for this target; supported targets: {SUPPORTED_CLOUDSYNC_TARGETS}"
    )]
    UnsupportedBundledCloudsync,
    /// `network_sync` drained {MAX_DRAIN} rounds and the hub still had pending
    /// changes. The site may be behind — returning a stale "synced" would be
    /// the silent-divergence bug SYNC-4 found (one `check` per call serves at
    /// most one blob). Fail loudly so the caller can retry/backoff.
    #[error("network_sync did not drain in {0} rounds; changes still pending")]
    DrainExhausted(usize),
    /// `network_sync` got a reply it could not parse for `receive.rows`. "the
    /// hub has nothing for me" and "I could not tell what the hub said" are
    /// different states — folding the latter into the former is the same
    /// silent-divergence class as not draining at all.
    #[error("network_sync got an unreadable sync reply: {0}")]
    UnreadableSyncReply(String),
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
            // A non-converging drain or an unreadable reply is not a transient
            // blip — retrying identically would loop forever or paper over a
            // protocol/schema drift. Surface it so the caller stops.
            Self::DrainExhausted(_) | Self::UnreadableSyncReply(_) => ErrorKind::Fatal,
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

// SQLite Cloud error code ranges:
//   < 10_000       SQLite native errors
//   10_000–99_999  SQLite Cloud server errors
//   >= 100_000     SDK/client internal errors
fn classify_error_code(code: i64) -> ErrorKind {
    match code {
        // SQLite Cloud server errors
        10004 => ErrorKind::Auth,                      // CLOUD_ERRCODE_AUTH
        10000 | 10003 | 10006 => ErrorKind::Transient, // MEM, INTERNAL, RAFT
        10001 | 10002 | 10005 => ErrorKind::Fatal,     // NOTFOUND, COMMAND, GENERIC

        // SDK internal errors
        100005 | 100008 => ErrorKind::Transient, // NETWORK, SOCKCLOSED
        100002 | 100003 | 100006 => ErrorKind::Fatal, // TLS, URL, FORMAT
        100000 | 100001 | 100004 | 100007 => ErrorKind::Transient, // GENERIC, PUBSUB, MEMORY, INDEX

        // SQLite native errors (< 10_000) are schema/constraint issues
        _ if code < 10_000 => ErrorKind::Fatal,

        // Unknown codes in cloud/sdk ranges — assume transient
        _ => ErrorKind::Transient,
    }
}
