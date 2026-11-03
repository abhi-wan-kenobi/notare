//! Contract between the sync transport (this crate) and the database side
//! (`crates/crsqlite` + `plugins/db`), for protocol v2.
//!
//! The transport never links SQLite. It moves [`WireChange`] values, which
//! mirror `crsqlite::changes::Change` field for field; `plugins/db` converts
//! between the two and implements [`ChangeSource`] over `crsql_changes`.
//!
//! Cursors are per remote peer and are stored by the database side (a local,
//! non-replicated table), never in the peer allowlist and never in a synced
//! table.

use serde::{Deserialize, Serialize};

/// A SQLite value as it appears in `crsql_changes.pk` / `.val`.
///
/// Deliberately a plain (externally tagged) enum: the wire format is postcard,
/// which cannot decode serde's `tag`/`content` representations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// One row of `crsql_changes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireChange {
    pub table: String,
    pub pk: WireValue,
    pub cid: String,
    pub val: WireValue,
    pub col_version: i64,
    pub db_version: i64,
    pub site_id: Vec<u8>,
    pub cl: i64,
    pub seq: i64,
}

/// Position in a peer's change stream: everything at or before
/// `(db_version, seq)` has been received and applied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WireCursor {
    pub db_version: i64,
    pub seq: i64,
}

/// A bounded page of changes after a cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WirePage {
    pub changes: Vec<WireChange>,
    /// Cursor to send in the next `Pull`.
    pub next: WireCursor,
    /// More changes exist after `next`.
    pub more: bool,
}

/// Errors the database side reports to the transport. The message is logged,
/// never sent to the remote peer.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ChangeSourceError(pub String);

/// Implemented by `plugins/db` over cr-sqlite. Every method is called from the
/// sync agent's tokio runtime.
pub trait ChangeSource: Send + Sync + 'static {
    /// This database's cr-sqlite site id (`crsql_site_id()`).
    fn site_id(&self) -> impl Future<Output = Result<Vec<u8>, ChangeSourceError>> + Send;

    /// Local changes after `after`, excluding rows whose `site_id` equals
    /// `exclude_site` (the requesting peer's own changes), at most `limit`.
    fn changes_since(
        &self,
        after: WireCursor,
        exclude_site: Option<Vec<u8>>,
        limit: usize,
    ) -> impl Future<Output = Result<WirePage, ChangeSourceError>> + Send;

    /// Apply a peer's changes in one transaction. Idempotent: re-applying a
    /// page that was already applied must be a no-op.
    fn apply(
        &self,
        changes: Vec<WireChange>,
    ) -> impl Future<Output = Result<(), ChangeSourceError>> + Send;

    /// Last cursor received from `peer` (the peer's z-base-32 device id);
    /// `WireCursor::default()` if never synced.
    fn peer_cursor(
        &self,
        peer: &str,
    ) -> impl Future<Output = Result<WireCursor, ChangeSourceError>> + Send;

    /// Persist the cursor for `peer` after its page has been applied.
    fn set_peer_cursor(
        &self,
        peer: &str,
        cursor: WireCursor,
    ) -> impl Future<Output = Result<(), ChangeSourceError>> + Send;
}
