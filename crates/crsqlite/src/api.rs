//! cr-sqlite (vlcn-io v0.16.3) function surface.
//!
//! Generic over `Executor` so both `&Pool` and `&mut Tx` call sites work.
//! Per the 2026-09-07 plan:
//!
//! - `is_crr` checks `<table>__crsql_clock` in `sqlite_master` (cr-sqlite
//!   has no "is this table a CRR" function).
//! - `load_probe` = `SELECT crsql_site_id()` — cr-sqlite v0.16.3 exposes no
//!   `crsql_version()` (probed 2026-09-07, recorded in sync-p2p.md §30), so
//!   site_id is the presence check.
//! - `begin_alter`/`commit_alter` back db-migrate's `MigrationScope::CrrAlter`
//!   (through db-core's `Db::begin_crr_alter_on`).
//! - `finalize` exists because cr-sqlite caches prepared statements per
//!   connection; the close-behaviour probe (tests/close_behaviour.rs) pinned
//!   the finalize-sweep-before-close pool policy.

use sqlx::{Executor, Sqlite};

use crate::error::Error;

/// Convert `table` to a CRDT-backed table (conflict-free replicated
/// relation). Idempotent — probed 2026-09-07 and pinned in
/// `tests/strict_roundtrip.rs` (`as_crr` twice is a no-op).
pub async fn as_crr<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT crsql_as_crr(?)")
        .bind(table_name)
        .execute(executor)
        .await?;
    Ok(())
}

/// Whether `table` is a CRR — `crsql_as_crr` creates a companion
/// `<table>__crsql_clock` table, so its presence in `sqlite_master` is the
/// check.
pub async fn is_crr<'e, E>(executor: E, table_name: &str) -> Result<bool, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = ? || '__crsql_clock'")
            .bind(table_name)
            .fetch_one(executor)
            .await?;
    Ok(count > 0)
}

/// This node's 16-byte site id. Also the extension load probe: with the
/// extension absent the query fails with "no such function".
pub async fn site_id<'e, E>(executor: E) -> Result<Vec<u8>, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT crsql_site_id()")
        .fetch_one(executor)
        .await?)
}

/// The current change-set version of the database. Monotonic; every write
/// to a CRR table bumps it.
pub async fn db_version<'e, E>(executor: E) -> Result<i64, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar("SELECT crsql_db_version()")
        .fetch_one(executor)
        .await?)
}

/// Extension load probe: `crsql_site_id()` resolves only when the cr-sqlite
/// extension is loaded on this connection. cr-sqlite v0.16.3 exposes no
/// `crsql_version()`, so this is the presence check (do not rely on one
/// existing).
pub async fn load_probe<'e, E>(executor: E) -> Result<bool, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result: Result<Vec<u8>, sqlx::Error> = sqlx::query_scalar("SELECT crsql_site_id()")
        .fetch_one(executor)
        .await;
    match result {
        Ok(site_id) if site_id.len() == 16 => Ok(true),
        Ok(site_id) => Err(Error::InvalidSiteId(site_id.len())),
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("no such function") => {
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

/// Begin a schema alteration on a CRR table (`crsql_begin_alter`). Between
/// begin and commit, changes to the table's schema are not tracked as CRDT
/// changes; db-migrate's `MigrationScope::CrrAlter` wraps every ALTER to a
/// synced CRR in this pair (via db-core's `Db::begin_crr_alter_on`).
pub async fn begin_alter<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT crsql_begin_alter(?)")
        .bind(table_name)
        .execute(executor)
        .await?;
    Ok(())
}

/// Commit a schema alteration begun with [`begin_alter`]
/// (`crsql_commit_alter`).
pub async fn commit_alter<'e, E>(executor: E, table_name: &str) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT crsql_commit_alter(?)")
        .bind(table_name)
        .execute(executor)
        .await?;
    Ok(())
}

/// Free the connection's cached CRDT statements (`SELECT crsql_finalize()`).
///
/// cr-sqlite caches prepared statements per connection; SQLite refuses to
/// close a connection with unfinalised statements, and sqlx panics when
/// `sqlite3_close` fails (sqlx-sqlite's `ConnectionHandle::drop`). Call this
/// on a connection right before it closes; [`close_pool`] does it for every
/// pooled connection.
pub async fn finalize<'e, E>(executor: E) -> Result<(), Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT crsql_finalize()")
        .execute(executor)
        .await?;
    Ok(())
}

/// How long [`close_pool`] waits, past which it logs a warning, for
/// checked-out connections to come back. Every connection is still
/// finalized before it closes — there is no fallback that skips finalize;
/// `pool.close()` on this crate's sqlx version blocks until every checked-
/// out connection returns anyway (`Pool::close` acquires every permit
/// before raw-closing idle connections), and a raw close without
/// `crsql_finalize` panics sqlx's worker thread.
const CLOSE_POOL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// Close a pool whose connections have the cr-sqlite extension loaded,
/// finalizing every connection first so no `sqlite3_close` fails.
///
/// Only idle connections are taken (`try_acquire` never opens a new one);
/// each is detached, finalized and closed individually. Checked-out
/// connections are waited for — callers stop the pool's users first so the
/// wait is normally zero; past [`CLOSE_POOL_DEADLINE`] a warning is logged
/// once so a caller that forgot to stop a user is visible, but the wait
/// continues rather than falling through to a raw `pool.close()`, which
/// would finalize nothing and risk the sqlx panic this function exists to
/// avoid. The pool must be built with `idle_timeout(None)` and
/// `max_lifetime(None)`: a connection the pool reaps on its own is closed
/// without being finalized.
pub async fn close_pool(pool: &sqlx::SqlitePool) {
    let deadline = tokio::time::Instant::now() + CLOSE_POOL_DEADLINE;
    let mut warned = false;
    while pool.size() > 0 && !pool.is_closed() {
        let Some(conn) = pool.try_acquire() else {
            if !warned && tokio::time::Instant::now() >= deadline {
                tracing::warn!(
                    "crsqlite::close_pool: still waiting on {} checked-out connection(s) \
                     after {CLOSE_POOL_DEADLINE:?}; a caller did not stop using the pool \
                     before closing it",
                    pool.size(),
                );
                warned = true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            continue;
        };
        let mut conn = conn.detach();
        if let Err(error) = finalize(&mut conn).await {
            tracing::warn!("crsql_finalize failed before close: {error}");
        }
        let _ = sqlx::Connection::close(conn).await;
    }
    pool.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `is_crr`'s SQL shape is safe to exercise without the extension: it
    /// probes `sqlite_master` only, so a plain connection answers `false`.
    /// (Keeps the query compiling/behaving on every CI run; the full engine
    /// surface is exercised by the un-gated engine tests.)
    #[tokio::test]
    async fn is_crr_reports_false_on_a_plain_connection() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .unwrap();

        assert!(!is_crr(&pool, "sessions").await.unwrap());
        assert!(!is_crr(&pool, "nope").await.unwrap());

        pool.close().await;
    }
}
