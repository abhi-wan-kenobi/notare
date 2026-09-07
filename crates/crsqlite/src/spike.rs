//! Shared harness for the Phase 0 spike tests.
//!
//! Everything in here is test-support code that both `strict_roundtrip.rs`
//! and `close_behaviour.rs` need: env-gated extension loading, real-schema
//! node setup, and the `SqlValue` codec.
//!
//! Design notes (deliberate, per the plan):
//!
//! - **Real schema, never hand-copied DDL.** Each node's tempdir DB is
//!   migrated by driving `hypr_db_app::APP_MIGRATION_STEPS` through the real
//!   `hypr_db_migrate::migrate` runner on a `Db::open` connection (WAL,
//!   `foreign_keys=ON`, sqlite-vec auto-registered — the embedding_chunks
//!   step creates a `vec0` virtual table). That yields the full app schema:
//!   21 STRICT tables, of which the 17 in the cloudsync table registry
//!   (18 counting `app_settings`, which the registry does not list) plus
//!   `embedding_vector_map` and the local-ledger tables.
//!
//! - **The `val` codec is the transport's codec.** The plan requires the
//!   spike to decode `crsql_changes.val` into a Rust enum via
//!   `SqliteValueRef::type_info()` — this exact code is what the 0.7
//!   transport will ship — so the spike exercises it rather than trusting
//!   SQL-level equality alone.
//!
//! - **sqlx worker-thread panic capture.** sqlx-sqlite runs each connection
//!   on a dedicated worker thread and `ConnectionHandle::drop` panics when
//!   `sqlite3_close` fails (`sqlx-sqlite/src/connection/handle.rs:136-145`;
//!   the repo has already met this class as the "(code: 5) unable to close
//!   due to unfinalized statements" teardown panic, sync-p2p.md §11.5).
//!   A panic *inside* a worker thread aborts the whole test process, so
//!   `close_behaviour.rs` drives each pool scenario on its own OS thread and
//!   reports through a channel.

use std::path::{Path, PathBuf};
use std::time::Instant;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

// The codec/row types the spike tests use moved to their permanent home in
// `crate::changes` (PR C-B); pull/apply stay here as pool-level wrappers
// over that API.
pub use crate::changes::{Change, SqlValue};

/// The spike's whole-history pull — every change with `db_version >
/// after_db_version`, excluding `exclude_site`'s own — expressed over the
/// transport's [`crate::changes::pull_changes`] Cursor API. The `seq:
/// i64::MAX` sentinel consumes all of `after_db_version`'s positions, so
/// only strictly newer db_versions are returned (the spike's original
/// `db_version > ?` semantics).
pub async fn pull_changes(
    pool: &SqlitePool,
    after_db_version: i64,
    exclude_site: &[u8],
) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let mut cursor = crate::Cursor {
        db_version: after_db_version,
        seq: i64::MAX,
    };
    loop {
        let page =
            crate::changes::pull_changes(pool, cursor, exclude_site, usize::MAX).await?;
        changes.extend(page.changes);
        if !page.more {
            return Ok(changes);
        }
        cursor = page.next;
    }
}

/// The spike's pool-level apply: one connection, one transaction (the
/// original spike's shape; the production shape composes the transport's
/// transaction around [`crate::changes::apply_changes`]).
pub async fn apply_changes(pool: &SqlitePool, changes: &[Change]) -> Result<()> {
    let mut tx = sqlx::Acquire::begin(pool).await?;
    crate::changes::apply_changes(&mut *tx, changes).await?;
    tx.commit().await?;
    Ok(())
}

/// The six tables enabled for sync (`SYNCED_TABLES` in
/// `crates/db-app/src/cloudsync.rs`), re-declared here: the const is private
/// to db-app, and the duplication is pinned by an assertion against the
pub const SYNCED_TABLES: &[&str] = &[
    "sessions",
    "session_documents",
    "transcripts",
    "action_items",
    "tags",
    "session_tags",
];

/// The 17 tables the cloudsync table registry lists
/// (`cloudsync_table_registry()` in `crates/db-app/src/cloudsync.rs`). The
/// spike converts exactly these to CRRs — this is the "all 18 STRICT tables"
/// obligation from the plan, phrased against the registry that actually
/// gates sync enablement (plus the probe table for BLOB/ANY coverage the app
/// schema cannot provide).
pub const REGISTRY_TABLES: &[&str] = &[
    "action_items",
    "calendars",
    "chat_groups",
    "chat_messages",
    "daily_notes",
    "entity_mentions",
    "events",
    "humans",
    "organizations",
    "session_attachments",
    "session_documents",
    "session_participants",
    "session_tags",
    "sessions",
    "tags",
    "templates",
    "transcripts",
];

/// STRICT table count the full migration set yields on a fresh node
/// (measured 2026-09-07 against main's 14 steps; asserted in
/// `strict_roundtrip.rs` via `PRAGMA table_list`, so a schema change that
/// adds or drops a STRICT table re-trips the spike).
pub const EXPECTED_STRICT_TABLE_COUNT: usize = 21;


/// Absolute path to the prebuilt cr-sqlite loadable extension, from
/// `CRSQLITE_SPIKE_EXTENSION`. `None` → tests print "skipped" and return
/// (the gating pattern of `SQLITECLOUD_URL` in `crates/db-core/tests/e2e.rs`).
///
/// The variable must hold an **absolute** path: SQLite resolves relative
/// extension paths against the process CWD, which is not stable under
/// `cargo test`.
pub fn extension_path_from_env() -> Option<PathBuf> {
    match std::env::var("CRSQLITE_SPIKE_EXTENSION") {
        Ok(value) if !value.trim().is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// Spike-local error: this crate deliberately carries no `thiserror`/`anyhow`
/// dependency; tests convert real errors into this and unwrap, keeping the
/// failure message in the assertion output.
#[derive(Debug)]
pub struct SpikeError(pub String);

impl std::fmt::Display for SpikeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SpikeError {}

impl From<sqlx::Error> for SpikeError {
    fn from(error: sqlx::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<crate::Error> for SpikeError {
    fn from(error: crate::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<std::io::Error> for SpikeError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

pub type Result<T, E = SpikeError> = std::result::Result<T, E>;

/// Connect options for a node: tempdir file, WAL, `foreign_keys=ON`, the
/// cr-sqlite extension loaded dynamically — the same style (and the same
/// `SqliteConnectOptions::extension()` API) as `hypr_cloudsync::apply`
/// (`crates/cloudsync/src/lib.rs`), which already proved this load path on
/// this exact sqlx stack.
pub fn connect_options(db_path: &Path, extension: &Path) -> SqliteConnectOptions {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);
    // SAFETY: the extension is the pinned vlcn-io v0.16.3 prebuilt release
    // binary pointed at by CRSQLITE_SPIKE_EXTENSION — a first-party file the
    // runner controls, the same trust domain as the vendored cloudsync
    // prebuilt this loading style was proven with. The crate-level
    // `#![forbid(unsafe_code)]` cannot be violated from this module without
    // the allow below being deleted in review, which is the point of keeping
    // it explicit.
    #[allow(unsafe_code)]
    unsafe {
        options.extension(extension.to_string_lossy().into_owned())
    }
}

/// A test node: a tempdir-backed pool with cr-sqlite loaded on every
/// connection, the real app schema migrated, and a `ChangeNotifier` wired
/// through `after_connect` exactly as `Db::open` wires it (needed by the
/// update-hook probe).
///
/// The pool is raw sqlx, not `hypr_db_core::Db`: db-core's cloudsync path is
/// the *old* engine, and its `Db` type has no public constructor over an
/// externally-built pool. Everything the plan requires the spike to exercise
/// (real migrations, notifier hooks, pooled close) is reachable this way.
pub struct Node {
    pub pool: SqlitePool,
    pub notifier: hypr_db_change::ChangeNotifier,
    pub label: &'static str,
    /// Dropped last, keeping the tempdir alive for the node's lifetime.
    #[allow(dead_code)]
    pub dir: tempfile::TempDir,
}

impl Node {
    /// Open a node with `max_connections` pooled connections.
    ///
    /// The schema is migrated on a short-lived plain `Db` (single
    /// connection, no extension) before the node's own pool opens — the
    /// extension is not needed to migrate, and migrating through the real
    /// `Db::open` path is exactly what the plan asks for.
    pub async fn open(
        label: &'static str,
        extension: &Path,
        max_connections: u32,
    ) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let db_path = dir.path().join(format!("{label}.db"));

        let migrate_db = hypr_db_core::Db::open(hypr_db_core::DbOpenOptions {
            storage: hypr_db_core::DbStorage::Local(&db_path),
            cloudsync_enabled: false,
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .map_err(|e| SpikeError(format!("Db::open: {e:?}")))?;
        hypr_db_migrate::migrate(&migrate_db, hypr_db_app::schema())
            .await
            .map_err(|e| SpikeError(format!("migrate: {e:?}")))?;
        migrate_db.pool().close().await;

        let options = connect_options(&db_path, extension);
        let (notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        let pool = pool_options
            .max_connections(max_connections)
            .connect_with(options)
            .await?;

        // Load probe: cr-sqlite v0.16.3 exposes no `crsql_version()` (probed
        // 2026-09-07; also absent upstream at this tag), so `crsql_site_id()`
        // is the presence check — the plan's `load_probe`.
        let site_id: Vec<u8> = sqlx::query_scalar("SELECT crsql_site_id()")
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            site_id.len(),
            16,
            "crsql_site_id() must return a 16-byte site id"
        );

        Ok(Self {
            pool,
            notifier,
            label,
            dir,
        })
    }

    pub async fn site_id(&self) -> Vec<u8> {
        sqlx::query_scalar("SELECT crsql_site_id()")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub async fn db_version(&self) -> i64 {
        sqlx::query_scalar("SELECT crsql_db_version()")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// `crsql_as_crr` on one table (idempotent — probed safe to call twice).
    pub async fn as_crr(&self, table: &str) -> Result<()> {
        sqlx::query("SELECT crsql_as_crr(?)")
            .bind(table)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Whether `<table>__crsql_clock` exists in `sqlite_master` — the
    /// plan's `is_crr` check.
    pub async fn is_crr(&self, table: &str) -> bool {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = ? || '__crsql_clock'",
        )
        .bind(table)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        count > 0
    }
}


/// `PRAGMA integrity_check` must report `ok`.
pub async fn assert_integrity_ok(pool: &SqlitePool, label: &str) {
    let result: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("integrity_check failed on {label}: {e}"));
    assert_eq!(result, "ok", "integrity_check on {label}");
}

/// A millisecond timer for the timed sections (800 KB pull+apply).
pub struct Timer {
    started: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }
}