mod crr;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub use crate::crr::CrrAlter;

#[derive(Clone, Copy, Debug)]
pub enum DbStorage<'a> {
    Local(&'a Path),
    Memory,
}

#[derive(Clone, Copy, Debug)]
pub struct DbOpenOptions<'a> {
    pub storage: DbStorage<'a>,
    pub journal_mode_wal: bool,
    pub foreign_keys: bool,
    pub max_connections: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum DbOpenError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[cfg(feature = "crsqlite")]
    #[error(transparent)]
    Crsqlite(#[from] hypr_crsqlite::Error),
}

pub type ManagedDb = std::sync::Arc<Db>;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Db {
    pub(crate) crsqlite_path: Option<PathBuf>,
    pub(crate) pool: SqlitePool,
    change_notifier: hypr_db_change::ChangeNotifier,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("crsqlite_path", &self.crsqlite_path)
            .field("change_notifier", &true)
            .finish_non_exhaustive()
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        // cr-sqlite caches prepared statements per connection; an
        // unfinalized `sqlite3_close` panics sqlx's worker thread
        // (sqlx-sqlite's `ConnectionHandle::drop`). `Db::close` finalizes
        // every connection first, but a caller that simply drops the `Db`
        // (forgets to call `close`, or unwinds before reaching it) bypasses
        // that. Best effort, not a guarantee: clone the pool handle (a
        // cheap `Arc` clone, so the field drop just below decrements a
        // refcount rather than tearing anything down) and finalize+close
        // it on a spawned task. This covers the common case — an app
        // whose tokio runtime keeps running past this point, which is
        // every real caller in this codebase — but not a process that
        // exits (or a runtime that shuts down) with zero further await
        // points on it; a caller that needs the guarantee must
        // `Db::close().await` itself before dropping.
        #[cfg(feature = "crsqlite")]
        if self.crsqlite_path.is_some() {
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    let pool = self.pool.clone();
                    handle.spawn(async move {
                        hypr_crsqlite::close_pool(&pool).await;
                    });
                }
                Err(_) => {
                    // Not inside any tokio runtime (a plain thread, a sync
                    // test teardown, or dropped after runtime shutdown) —
                    // there is nothing to spawn onto, so finalize
                    // synchronously on a throwaway current-thread runtime
                    // rather than let the connections close raw.
                    let pool = self.pool.clone();
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt.block_on(hypr_crsqlite::close_pool(&pool)),
                        Err(error) => {
                            tracing::error!(
                                "Db with cr-sqlite loaded was dropped outside a tokio \
                                 runtime and a fallback runtime could not be built \
                                 ({error}); connections will close without \
                                 crsql_finalize and may panic sqlx's worker thread — \
                                 call Db::close before dropping"
                            );
                        }
                    }
                }
            }
        }
    }
}

impl Db {
    pub async fn open(options: DbOpenOptions<'_>) -> Result<Self, DbOpenError> {
        let (change_notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        connect_with_options(&options, pool_options, change_notifier, false).await
    }

    /// Open with the cr-sqlite engine loaded on every pooled connection.
    ///
    /// Loading makes no table a CRR — that is `crsqlite::as_crr`, a separate
    /// opt-in. The pool never recycles connections on its own (no idle
    /// timeout, no max lifetime): cr-sqlite's per-connection statement cache
    /// must be finalized before `sqlite3_close`, so connections may only
    /// close through [`Db::close`].
    #[cfg(feature = "crsqlite")]
    pub async fn open_crsqlite(options: DbOpenOptions<'_>) -> Result<Self, DbOpenError> {
        let (change_notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        connect_with_options(&options, pool_options, change_notifier, true).await
    }

    /// Path of the loaded cr-sqlite extension, if this db was opened with
    /// [`Db::open_crsqlite`].
    pub fn crsqlite_path(&self) -> Option<&Path> {
        self.crsqlite_path.as_deref()
    }

    /// Close the pool. With cr-sqlite loaded, every connection is finalized
    /// first; a plain `pool().close()` on such a db panics sqlx's worker
    /// threads ("unable to close due to unfinalized statements"). Stop every
    /// user of the pool (live queries, sync sessions) before calling.
    pub async fn close(&self) {
        #[cfg(feature = "crsqlite")]
        if self.crsqlite_path.is_some() {
            hypr_crsqlite::close_pool(&self.pool).await;
            return;
        }
        self.pool.close().await;
    }

    pub fn change_notifier(&self) -> &hypr_db_change::ChangeNotifier {
        &self.change_notifier
    }

    pub async fn connect_local_plain(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
        }
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let (change_notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        let pool = pool_options.connect_with(options).await?;

        Ok(Self {
            crsqlite_path: None,
            pool,
            change_notifier,
        })
    }

    pub async fn connect_local_read_only(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let options = apply_internal_connect_policy(SqliteConnectOptions::new())
            .filename(path)
            .read_only(true)
            .pragma("foreign_keys", "ON")
            .pragma("query_only", "ON");
        let (change_notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        let pool = pool_options.connect_with(options).await?;

        Ok(Self {
            crsqlite_path: None,
            pool,
            change_notifier,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options =
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?)
                .pragma("foreign_keys", "ON");
        let (change_notifier, pool_options) = hypr_db_change::ChangeNotifier::new();
        let pool = pool_options
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self {
            crsqlite_path: None,
            pool,
            change_notifier,
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

async fn connect_with_options(
    options: &DbOpenOptions<'_>,
    pool_options: SqlitePoolOptions,
    change_notifier: hypr_db_change::ChangeNotifier,
    crsqlite: bool,
) -> Result<Db, DbOpenError> {
    let mut connect_options = match options.storage {
        DbStorage::Local(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            apply_internal_connect_policy(SqliteConnectOptions::new())
                .filename(path)
                .create_if_missing(true)
        }
        DbStorage::Memory => {
            apply_internal_connect_policy(SqliteConnectOptions::from_str("sqlite::memory:")?)
        }
    };

    if options.journal_mode_wal {
        connect_options = connect_options.pragma("journal_mode", "WAL");
    }
    if options.foreign_keys {
        connect_options = connect_options.pragma("foreign_keys", "ON");
    }

    #[cfg(feature = "crsqlite")]
    let (connect_options, crsqlite_path) = if crsqlite {
        let (connect_options, crsqlite_path) = hypr_crsqlite::apply(connect_options)?;
        (connect_options, Some(crsqlite_path))
    } else {
        (connect_options, None)
    };
    #[cfg(not(feature = "crsqlite"))]
    let crsqlite_path: Option<PathBuf> = None;

    let mut pool_options = pool_options;
    match options.storage {
        DbStorage::Memory => {
            pool_options = pool_options.max_connections(1);
        }
        DbStorage::Local(_) => {
            if let Some(max) = options.max_connections {
                pool_options = pool_options.max_connections(max);
            }
        }
    };

    if crsqlite {
        pool_options = pool_options.idle_timeout(None).max_lifetime(None);
    }
    let pool = pool_options.connect_with(connect_options).await?;

    Ok(Db {
        crsqlite_path,
        pool,
        change_notifier,
    })
}

fn apply_internal_connect_policy(connect_options: SqliteConnectOptions) -> SqliteConnectOptions {
    register_sqlite_vec();
    connect_options.busy_timeout(SQLITE_BUSY_TIMEOUT)
}

static SQLITE_VEC_INIT: Once = Once::new();

/// Register the `sqlite-vec` extension as a SQLite auto-extension exactly once
/// per process, BEFORE any pool is created. `sqlite3_auto_extension` installs
/// the init hook globally, so every connection SQLite opens afterwards — via
/// any of db-core's connect paths, and the migration runner that rides on
/// them — has the `vec0` virtual table and `vec_*` functions available.
///
/// Static registration (vs `SqliteConnectOptions::extension("vec0")`) is
/// deliberate: it needs no loadable dylib, so nothing new has to be shipped or
/// notarized on macOS (S0 spike decision, 2026-07-22). Verified on the
/// workspace stack (sqlx 0.9.0-alpha.1 + sqlite-unbundled + libsqlite3-sys
/// 0.35): the auto-extension fires on sqlx-opened connections.
fn register_sqlite_vec() {
    SQLITE_VEC_INIT.call_once(|| {
        // SAFETY: `sqlite3_vec_init` is the extension entry point provided by
        // the `sqlite-vec` crate; `sqlite3_auto_extension` takes a C function
        // pointer to it. The explicit transmute annotation matches the
        // `sqlite3_auto_extension` argument type (required by
        // clippy::missing_transmute_annotations). Called once, before any
        // connection is opened.
        unsafe {
            libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut libsqlite3_sys::sqlite3,
                    *mut *mut std::os::raw::c_char,
                    *const libsqlite3_sys::sqlite3_api_routines,
                ) -> std::os::raw::c_int,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_local_plain_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("nonexistent").join("nested").join("app.db");
        let db = Db::connect_local_plain(&db_path).await.unwrap();
        assert!(db_path.exists());
        drop(db);
    }

    /// The sqlite-vec auto-extension is live on connections db-core opens, so a
    /// `vec0` virtual table can be created and KNN-queried (the WS-B1 backend
    /// contract). This is the guarantee migrations and the embedding-search
    /// plugin rely on.
    #[tokio::test]
    async fn sqlite_vec_extension_is_registered_on_connections() {
        let db = Db::connect_memory_plain().await.unwrap();

        let version: String = sqlx::query_scalar("SELECT vec_version()")
            .fetch_one(db.pool())
            .await
            .expect("vec_version() should resolve when sqlite-vec is registered");
        assert!(
            version.starts_with('v'),
            "unexpected vec version: {version}"
        );

        sqlx::query("CREATE VIRTUAL TABLE t USING vec0(embedding float[4])")
            .execute(db.pool())
            .await
            .unwrap();
        for (id, e) in [
            (1i64, [0.1f32, 0.1, 0.1, 0.1]),
            (2, [0.9, 0.9, 0.9, 0.9]),
            (3, [0.2, 0.15, 0.1, 0.05]),
        ] {
            let bytes: Vec<u8> = e.iter().flat_map(|f| f.to_le_bytes()).collect();
            sqlx::query("INSERT INTO t(rowid, embedding) VALUES (?, ?)")
                .bind(id)
                .bind(bytes)
                .execute(db.pool())
                .await
                .unwrap();
        }
        let query: Vec<u8> = [0.12f32, 0.11, 0.1, 0.09]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let nearest: i64 = sqlx::query_scalar(
            "SELECT rowid FROM t WHERE embedding MATCH ? ORDER BY distance LIMIT 1",
        )
        .bind(query)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(nearest, 1, "KNN should return the closest vector");
    }

    #[tokio::test]
    async fn connect_local_read_only_does_not_create_missing_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("missing.db");

        let result = Db::connect_local_read_only(&db_path).await;

        assert!(result.is_err());
        assert!(!db_path.exists());
    }

    #[tokio::test]
    async fn connect_local_read_only_rejects_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");
        let writable = Db::connect_local_plain(&db_path).await.unwrap();
        sqlx::query("CREATE TABLE records (id TEXT PRIMARY KEY NOT NULL)")
            .execute(writable.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO records (id) VALUES ('existing')")
            .execute(writable.pool())
            .await
            .unwrap();
        writable.pool().close().await;

        let read_only = Db::connect_local_read_only(&db_path).await.unwrap();
        let rows: Vec<String> = sqlx::query_scalar("SELECT id FROM records ORDER BY id")
            .fetch_all(read_only.pool())
            .await
            .unwrap();
        let query_only: i64 = sqlx::query_scalar("PRAGMA query_only")
            .fetch_one(read_only.pool())
            .await
            .unwrap();
        let write_result = sqlx::query("INSERT INTO records (id) VALUES ('rejected')")
            .execute(read_only.pool())
            .await;

        assert_eq!(rows, vec!["existing"]);
        assert_eq!(query_only, 1);
        assert!(write_result.is_err());
    }

    #[tokio::test]
    async fn open_applies_requested_pragmas() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");

        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(1),
        })
        .await
        .unwrap();

        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(busy_timeout, SQLITE_BUSY_TIMEOUT.as_millis() as i64);
    }

    #[tokio::test]
    async fn emits_table_changes_for_local_writes() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();
        let before = notifier.current_seq();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(db.pool())
            .await
            .unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, hypr_db_change::TableChangeKind::Insert);
        assert!(change.seq > before);
        assert_eq!(notifier.current_seq(), change.seq);
        assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
    }

    #[tokio::test]
    async fn emits_table_changes_only_after_commit() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(&mut *tx)
            .await
            .unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );

        tx.commit().await.unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, hypr_db_change::TableChangeKind::Insert);
        assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
    }

    #[tokio::test]
    async fn rollback_clears_pending_table_changes() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(&mut *tx)
            .await
            .unwrap();

        tx.rollback().await.unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn coalesces_multiple_writes_in_a_transaction() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();
        let mut tx = db.pool().begin().await.unwrap();

        sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
            .execute(&mut *tx)
            .await
            .unwrap();

        tx.commit().await.unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(change.table, "test_events");
        assert_eq!(change.kind, hypr_db_change::TableChangeKind::Update);
        assert_eq!(notifier.latest_table_seq("test_events"), Some(change.seq));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), changes.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn emits_update_and_delete_table_changes() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO test_events (id, value) VALUES ('a', 'before')")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();

        sqlx::query("UPDATE test_events SET value = 'after' WHERE id = 'a'")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM test_events WHERE id = 'a'")
            .execute(db.pool())
            .await
            .unwrap();

        let update = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        let delete = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(update.table, "test_events");
        assert_eq!(update.kind, hypr_db_change::TableChangeKind::Update);
        assert_eq!(delete.table, "test_events");
        assert_eq!(delete.kind, hypr_db_change::TableChangeKind::Delete);
        assert!(delete.seq > update.seq);
        assert_eq!(notifier.latest_table_seq("test_events"), Some(delete.seq));
    }

    #[tokio::test]
    async fn emits_table_changes_across_multiple_connections() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");

        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Local(&db_path),
            journal_mode_wal: true,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE multi_conn_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let mut changes = notifier.subscribe();
        let mut conn_a = db.pool().acquire().await.unwrap();
        let mut conn_b = db.pool().acquire().await.unwrap();

        sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('a')")
            .execute(&mut *conn_a)
            .await
            .unwrap();
        sqlx::query("INSERT INTO multi_conn_events (id) VALUES ('b')")
            .execute(&mut *conn_b)
            .await
            .unwrap();

        let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first.table, "multi_conn_events");
        assert_eq!(second.table, "multi_conn_events");
        assert_ne!(first.seq, second.seq);
    }

    #[tokio::test]
    async fn tracks_monotonic_change_sequences_per_table() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier();
        sqlx::query("CREATE TABLE test_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE other_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let start = notifier.current_seq();
        let mut changes = notifier.subscribe();

        sqlx::query("INSERT INTO test_events (id) VALUES ('a')")
            .execute(db.pool())
            .await
            .unwrap();
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        sqlx::query("INSERT INTO test_events (id) VALUES ('b')")
            .execute(db.pool())
            .await
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        sqlx::query("INSERT INTO other_events (id) VALUES ('c')")
            .execute(db.pool())
            .await
            .unwrap();
        let third = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(first.seq > start);
        assert!(second.seq > first.seq);
        assert!(third.seq > second.seq);
        assert_eq!(notifier.current_seq(), third.seq);
        assert_eq!(notifier.latest_table_seq("test_events"), Some(second.seq));
        assert_eq!(notifier.latest_table_seq("other_events"), Some(third.seq));
        assert_eq!(notifier.latest_table_seq("missing_events"), None);
    }

    #[tokio::test]
    async fn notifier_survives_db_drop() {
        let db = Db::connect_memory_plain().await.unwrap();
        let notifier = db.change_notifier().clone();
        sqlx::query("CREATE TABLE retained_events (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let pool = db.pool().clone();
        let mut changes = notifier.subscribe();
        drop(db);

        sqlx::query("INSERT INTO retained_events (id) VALUES ('a')")
            .execute(&pool)
            .await
            .unwrap();

        let change = tokio::time::timeout(std::time::Duration::from_secs(1), changes.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(change.table, "retained_events");
        assert_eq!(change.kind, hypr_db_change::TableChangeKind::Insert);
        assert_eq!(
            notifier.latest_table_seq("retained_events"),
            Some(change.seq)
        );
    }

    #[tokio::test]
    async fn open_memory_clamps_max_connections_to_one() {
        let db = Db::open(DbOpenOptions {
            storage: DbStorage::Memory,
            journal_mode_wal: false,
            foreign_keys: true,
            max_connections: Some(4),
        })
        .await
        .unwrap();

        let _conn = db.pool().acquire().await.unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), db.pool().acquire())
                .await
                .is_err()
        );
    }
}
