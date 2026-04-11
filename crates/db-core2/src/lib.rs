#![forbid(unsafe_code)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;

pub use hypr_cloudsync::Error;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[derive(Clone, Copy, Debug)]
pub enum DbStorage<'a> {
    Local(&'a Path),
    Memory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationFailurePolicy {
    Fail,
    Recreate,
}

#[derive(Clone, Copy, Debug)]
pub struct DbOpenOptions<'a> {
    pub storage: DbStorage<'a>,
    pub cloudsync: bool,
    pub journal_mode_wal: bool,
    pub foreign_keys: bool,
    pub max_connections: Option<u32>,
    pub migration_failure_policy: MigrationFailurePolicy,
}

#[derive(Debug, thiserror::Error)]
pub enum DbOpenError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Cloudsync(#[from] hypr_cloudsync::Error),
    #[error("migration failed: {0}")]
    Migration(String),
    #[error("failed to recreate database after migration failure: {0}")]
    RecreateFailed(String),
}

#[derive(Debug)]
pub struct Db3 {
    cloudsync_path: Option<PathBuf>,
    pool: SqlitePool,
}

type BoxedMigrationFuture<'a, E> = Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'a>>;

impl Db3 {
    pub async fn open_with_migrate<F, E>(
        options: DbOpenOptions<'_>,
        migrate: F,
    ) -> Result<Self, DbOpenError>
    where
        F: for<'a> Fn(&'a SqlitePool) -> BoxedMigrationFuture<'a, E>,
        E: std::fmt::Display,
    {
        match try_open_with_migrate(&options, &migrate).await {
            Ok(db) => Ok(db),
            Err(DbOpenError::Migration(message))
                if matches!(
                    options.migration_failure_policy,
                    MigrationFailurePolicy::Recreate
                ) =>
            {
                tracing::warn!("database migration failed, recreating fresh database: {message}");
                recreate_storage(&options)?;
                try_open_with_migrate(&options, &migrate)
                    .await
                    .map_err(|error| DbOpenError::RecreateFailed(error.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn connect_local(path: impl AsRef<Path>) -> Result<Self, Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_memory() -> Result<Self, Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let (options, cloudsync_path) = hypr_cloudsync::apply(options)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(Error::from)?;

        Ok(Self {
            cloudsync_path: Some(cloudsync_path),
            pool,
        })
    }

    pub async fn connect_local_plain(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .pragma("foreign_keys", "ON");
        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub async fn connect_memory_plain() -> Result<Self, sqlx::Error> {
        let options =
            SqliteConnectOptions::from_str("sqlite::memory:")?.pragma("foreign_keys", "ON");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self {
            cloudsync_path: None,
            pool,
        })
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&Path> {
        self.cloudsync_path.as_deref()
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn cloudsync_version(&self) -> Result<String, Error> {
        hypr_cloudsync::version(&self.pool).await
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        force: Option<bool>,
    ) -> Result<(), Error> {
        hypr_cloudsync::init(&self.pool, table_name, crdt_algo, force).await
    }

    pub async fn cloudsync_network_init(&self, connection_string: &str) -> Result<(), Error> {
        hypr_cloudsync::network_init(&self.pool, connection_string).await
    }

    pub async fn cloudsync_network_set_apikey(&self, api_key: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_apikey(&self.pool, api_key).await
    }

    pub async fn cloudsync_network_set_token(&self, token: &str) -> Result<(), Error> {
        hypr_cloudsync::network_set_token(&self.pool, token).await
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<(), Error> {
        hypr_cloudsync::network_sync(&self.pool, wait_ms, max_retries).await
    }
}

async fn try_open_with_migrate<F, E>(
    options: &DbOpenOptions<'_>,
    migrate: &F,
) -> Result<Db3, DbOpenError>
where
    F: for<'a> Fn(&'a SqlitePool) -> BoxedMigrationFuture<'a, E>,
    E: std::fmt::Display,
{
    let db = connect_with_options(options).await?;

    if let Err(error) = migrate(db.pool()).await {
        db.pool.clone().close().await;
        return Err(DbOpenError::Migration(error.to_string()));
    }

    Ok(db)
}

async fn connect_with_options(options: &DbOpenOptions<'_>) -> Result<Db3, DbOpenError> {
    let mut connect_options = match options.storage {
        DbStorage::Local(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
        }
        DbStorage::Memory => SqliteConnectOptions::from_str("sqlite::memory:")?,
    };

    if options.journal_mode_wal {
        connect_options = connect_options.pragma("journal_mode", "WAL");
    }
    if options.foreign_keys {
        connect_options = connect_options.pragma("foreign_keys", "ON");
    }

    let (connect_options, cloudsync_path) = if options.cloudsync {
        let (connect_options, cloudsync_path) = hypr_cloudsync::apply(connect_options)?;
        (connect_options, Some(cloudsync_path))
    } else {
        (connect_options, None)
    };

    let mut pool_options = SqlitePoolOptions::new();
    if let Some(max_connections) = options.max_connections {
        pool_options = pool_options.max_connections(max_connections);
    }

    let pool = pool_options.connect_with(connect_options).await?;

    Ok(Db3 {
        cloudsync_path,
        pool,
    })
}

fn recreate_storage(options: &DbOpenOptions<'_>) -> Result<(), DbOpenError> {
    match options.storage {
        DbStorage::Local(path) => {
            wipe_db_file(path);
            if options.cloudsync {
                let connect_options = SqliteConnectOptions::new().filename(path);
                let (_, cloudsync_path) = hypr_cloudsync::apply(connect_options)?;
                wipe_db_file(&cloudsync_path);
            }
        }
        DbStorage::Memory => {}
    }

    Ok(())
}

fn wipe_db_file(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let file = PathBuf::from(format!("{}{suffix}", path.display()));
        if file.exists() {
            let _ = std::fs::remove_file(file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn connect_local_plain_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("nonexistent").join("nested").join("app.db");
        let db = Db3::connect_local_plain(&db_path).await.unwrap();
        assert!(db_path.exists());
        drop(db);
    }

    #[tokio::test]
    async fn open_with_migrate_recreates_local_db_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("app.db");
        let attempts = AtomicUsize::new(0);

        let db = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Local(&db_path),
                cloudsync: false,
                journal_mode_wal: true,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Recreate,
            },
            |pool| {
                let n = attempts.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if n == 0 {
                        sqlx::query("CREATE TABLE broken (id TEXT PRIMARY KEY NOT NULL)")
                            .execute(pool)
                            .await
                            .unwrap();
                        Err("boom")
                    } else {
                        sqlx::query("CREATE TABLE fresh (id TEXT PRIMARY KEY NOT NULL)")
                            .execute(pool)
                            .await
                            .unwrap();
                        Ok::<(), &'static str>(())
                    }
                })
            },
        )
        .await
        .unwrap();

        let tables: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(tables, vec!["fresh"]);
    }

    #[tokio::test]
    async fn open_with_migrate_returns_migration_error_when_fail_policy_is_used() {
        let error = Db3::open_with_migrate(
            DbOpenOptions {
                storage: DbStorage::Memory,
                cloudsync: false,
                journal_mode_wal: false,
                foreign_keys: true,
                max_connections: Some(1),
                migration_failure_policy: MigrationFailurePolicy::Fail,
            },
            |_pool| Box::pin(async { Err::<(), _>("nope") }),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, DbOpenError::Migration(message) if message == "nope"));
    }
}
