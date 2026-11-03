use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::Db;

/// An open `crsql_begin_alter` window on one pooled connection, started by
/// [`Db::begin_crr_alter_on`]. Run the DDL on that same connection, then hand
/// the connection to [`CrrAlter::commit`] — or [`CrrAlter::abort`] if the DDL
/// failed.
///
/// Both close the connection. cr-sqlite v0.16.3 caches each connection's
/// view of a CRR's columns and never refreshes it after an alter (probed
/// 2026-11-04): once a window commits, an `UPDATE` fails with "expected 5
/// values, got 7" on the connection that ran it and on every connection that
/// wrote to the table before it, and after a failed window the rolled-back
/// table still produces `crsql_changes` rows for the dropped column. Only
/// connections opened afterwards see the real schema, so a window must run
/// while nothing else holds a pooled connection — migrations at open.
#[must_use = "an open alter window must be committed or aborted with the connection that began it"]
#[derive(Debug)]
pub struct CrrAlter {
    #[cfg(feature = "crsqlite")]
    table_name: String,
    #[cfg(feature = "crsqlite")]
    pool: sqlx::SqlitePool,
    // Without cr-sqlite no window can ever be opened; the uninhabited field
    // makes that a compile-time fact instead of a runtime assumption.
    #[cfg(not(feature = "crsqlite"))]
    never: std::convert::Infallible,
}

impl Db {
    /// Begin a cr-sqlite alter window for DDL on `table_name`, on `conn`.
    ///
    /// cr-sqlite tracks a CRR through triggers and a `<table>__crsql_clock`
    /// table built from the table's columns; DDL that runs outside
    /// `crsql_begin_alter`/`crsql_commit_alter` leaves them describing the
    /// old schema. Returns `None` — run the DDL plainly — when this db has no
    /// cr-sqlite loaded or `table_name` is not a CRR yet: tables become CRRs
    /// only when the user turns sync on, and `crsql_commit_alter` must not be
    /// what converts one.
    ///
    /// Fails on an in-memory database, which cannot survive the connection
    /// recycling a window ends with. If `crsql_begin_alter` itself fails,
    /// `conn` is finalized and closes when dropped.
    pub async fn begin_crr_alter_on(
        &self,
        conn: &mut PoolConnection<Sqlite>,
        table_name: &str,
    ) -> Result<Option<CrrAlter>, sqlx::Error> {
        #[cfg(feature = "crsqlite")]
        {
            if self.crsqlite_path.is_none()
                || !hypr_crsqlite::is_crr(&mut **conn, table_name)
                    .await
                    .map_err(into_sqlx)?
            {
                return Ok(None);
            }
            let file: String =
                sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
                    .fetch_one(&mut **conn)
                    .await?;
            if file.is_empty() {
                return Err(sqlx::Error::Configuration(
                    format!(
                        "cannot alter CRR {table_name} on an in-memory database: the alter \
                         window must close every connection afterwards"
                    )
                    .into(),
                ));
            }
            if let Err(error) = hypr_crsqlite::begin_alter(&mut **conn, table_name).await {
                finalize(conn).await;
                conn.close_on_drop();
                return Err(into_sqlx(error));
            }
            Ok(Some(CrrAlter {
                table_name: table_name.to_string(),
                pool: self.pool.clone(),
            }))
        }
        #[cfg(not(feature = "crsqlite"))]
        {
            let _ = (conn, table_name);
            Ok(None)
        }
    }
}

impl CrrAlter {
    /// Close the window (`crsql_commit_alter`), rebuilding the CRR's
    /// triggers and clock for the new schema, then close `conn` and every
    /// idle pooled connection so the next ones load the new schema. A failed
    /// commit still closes `conn`, which rolls the window back.
    pub async fn commit(self, mut conn: PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
        #[cfg(feature = "crsqlite")]
        {
            let result = hypr_crsqlite::commit_alter(&mut *conn, &self.table_name).await;
            close(conn).await;
            result.map_err(into_sqlx)?;
            for _ in 0..self.pool.num_idle() {
                let Some(idle) = self.pool.try_acquire() else {
                    break;
                };
                close(idle).await;
            }
            Ok(())
        }
        #[cfg(not(feature = "crsqlite"))]
        {
            let _ = &mut conn;
            match self.never {}
        }
    }

    /// Abandon the window after its DDL failed. Closing `conn` rolls the
    /// window back.
    pub async fn abort(self, conn: PoolConnection<Sqlite>) {
        #[cfg(feature = "crsqlite")]
        close(conn).await;
        #[cfg(not(feature = "crsqlite"))]
        {
            let _ = conn;
            match self.never {}
        }
    }
}

/// Every cr-sqlite connection must be finalized before it closes.
#[cfg(feature = "crsqlite")]
async fn finalize(conn: &mut sqlx::SqliteConnection) {
    if let Err(error) = hypr_crsqlite::finalize(conn).await {
        tracing::warn!("crsql_finalize failed before closing a connection: {error}");
    }
}

#[cfg(feature = "crsqlite")]
async fn close(conn: PoolConnection<Sqlite>) {
    let mut conn = conn.detach();
    finalize(&mut conn).await;
    if let Err(error) = sqlx::Connection::close(conn).await {
        tracing::warn!("closing a connection after a cr-sqlite alter window: {error}");
    }
}

#[cfg(feature = "crsqlite")]
fn into_sqlx(error: hypr_crsqlite::Error) -> sqlx::Error {
    match error {
        hypr_crsqlite::Error::Sqlx(error) => error,
        other => sqlx::Error::config(other),
    }
}
