//! `sync_p2p::ChangeSource` over cr-sqlite: the database half of protocol v2.
//!
//! The transport moves `WireChange`s; this module converts them to and from
//! `hypr_crsqlite::Change` field for field and keeps the per-peer pull
//! cursors in `sync_peer_cursors` (a plain local table, never a CRR).

use std::sync::Arc;

use hypr_crsqlite::{Change, Cursor, SqlValue};
use hypr_db_core::Db;
use sync_p2p::{ChangeSource, ChangeSourceError, WireChange, WireCursor, WirePage, WireValue};

#[derive(Clone)]
pub struct CrsqliteChangeSource {
    db: Arc<Db>,
}

impl CrsqliteChangeSource {
    /// `db` must have been opened with `Db::open_crsqlite`, with the synced
    /// tables already converted (see [`enable_crr_tables`]).
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }
}

/// `crsql_as_crr` on every table in [`hypr_db_app::SYNCED_TABLES`].
/// Idempotent; rows that existed before conversion are backfilled into the
/// change stream by cr-sqlite itself.
pub async fn enable_crr_tables(db: &Db) -> Result<(), hypr_crsqlite::Error> {
    for table in hypr_db_app::SYNCED_TABLES {
        hypr_crsqlite::as_crr(db.pool(), table).await?;
    }
    Ok(())
}

fn source_error(error: impl std::fmt::Display) -> ChangeSourceError {
    ChangeSourceError(error.to_string())
}

fn to_sql(value: WireValue) -> SqlValue {
    match value {
        WireValue::Null => SqlValue::Null,
        WireValue::Integer(v) => SqlValue::Integer(v),
        WireValue::Real(v) => SqlValue::Real(v),
        WireValue::Text(v) => SqlValue::Text(v),
        WireValue::Blob(v) => SqlValue::Blob(v),
    }
}

fn to_wire(value: SqlValue) -> WireValue {
    match value {
        SqlValue::Null => WireValue::Null,
        SqlValue::Integer(v) => WireValue::Integer(v),
        SqlValue::Real(v) => WireValue::Real(v),
        SqlValue::Text(v) => WireValue::Text(v),
        SqlValue::Blob(v) => WireValue::Blob(v),
    }
}

fn change_from_wire(change: WireChange) -> Change {
    Change {
        table: change.table,
        pk: to_sql(change.pk),
        cid: change.cid,
        val: to_sql(change.val),
        col_version: change.col_version,
        db_version: change.db_version,
        site_id: change.site_id,
        cl: change.cl,
        seq: change.seq,
    }
}

fn change_to_wire(change: Change) -> WireChange {
    WireChange {
        table: change.table,
        pk: to_wire(change.pk),
        cid: change.cid,
        val: to_wire(change.val),
        col_version: change.col_version,
        db_version: change.db_version,
        site_id: change.site_id,
        cl: change.cl,
        seq: change.seq,
    }
}

impl ChangeSource for CrsqliteChangeSource {
    async fn site_id(&self) -> Result<Vec<u8>, ChangeSourceError> {
        hypr_crsqlite::site_id(self.db.pool())
            .await
            .map_err(source_error)
    }

    async fn changes_since(
        &self,
        after: WireCursor,
        exclude_site: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<WirePage, ChangeSourceError> {
        let page = hypr_crsqlite::pull_changes(
            self.db.pool(),
            Cursor {
                db_version: after.db_version,
                seq: after.seq,
            },
            exclude_site.as_deref(),
            limit,
        )
        .await
        .map_err(source_error)?;

        Ok(WirePage {
            changes: page.changes.into_iter().map(change_to_wire).collect(),
            next: WireCursor {
                db_version: page.next.db_version,
                seq: page.next.seq,
            },
            more: page.more,
        })
    }

    async fn apply(&self, changes: Vec<WireChange>) -> Result<(), ChangeSourceError> {
        if changes.is_empty() {
            return Ok(());
        }
        let changes: Vec<Change> = changes.into_iter().map(change_from_wire).collect();
        let mut tx = self.db.pool().begin().await.map_err(source_error)?;
        hypr_crsqlite::apply_changes(&mut tx, &changes)
            .await
            .map_err(source_error)?;
        tx.commit().await.map_err(source_error)
    }

    async fn peer_cursor(&self, peer: &str) -> Result<WireCursor, ChangeSourceError> {
        let row: Option<(i64, i64)> =
            sqlx::query_as("SELECT db_version, seq FROM sync_peer_cursors WHERE peer_id = ?")
                .bind(peer)
                .fetch_optional(self.db.pool())
                .await
                .map_err(source_error)?;
        Ok(
            row.map_or_else(WireCursor::default, |(db_version, seq)| WireCursor {
                db_version,
                seq,
            }),
        )
    }

    async fn set_peer_cursor(
        &self,
        peer: &str,
        cursor: WireCursor,
    ) -> Result<(), ChangeSourceError> {
        sqlx::query(
            "INSERT INTO sync_peer_cursors (peer_id, db_version, seq, updated_at)
             VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(peer_id) DO UPDATE SET
               db_version = excluded.db_version,
               seq = excluded.seq,
               updated_at = excluded.updated_at",
        )
        .bind(peer)
        .bind(cursor.db_version)
        .bind(cursor.seq)
        .execute(self.db.pool())
        .await
        .map_err(source_error)?;
        Ok(())
    }
}
