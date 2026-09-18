//! The DB ↔ agent seam: [`ChangeSource`].
//!
//! `sync-p2p` owns the wire protocol and the iroh transport, and speaks to
//! the CRDT engine **only** through this trait. The concrete implementation
//! over the real database (`DbChangeSource`) lives above this crate (see the
//! plan: `plugins/db/src/sync.rs`, PR slice C-E) so that `sync-p2p` depends on
//! no SQLite engine crate — `cargo test -p sync-p2p` needs no SQLite at all,
//! and every test in this crate drives a [`FakeSource`].
//!
//! The change/cursor/page types here mirror the engine crate's (`hypr-crsqlite`
//! `changes.rs`) field-for-field; the concrete source converts between the
//! two, so the wire contract is defined once, at the seam.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// A single SQLite value, by storage class — the exact codec shape the cr-sqlite
/// engine crate (`hypr-crsqlite` `changes.rs`) defines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// One `crsql_changes` row, as the wire ships it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub table: String,
    pub pk: SqlValue,
    pub cid: String,
    pub val: SqlValue,
    pub col_version: i64,
    pub db_version: i64,
    pub site_id: Vec<u8>,
    pub cl: i64,
    pub seq: i64,
}

/// The per-peer pull position: changes are ordered by `(db_version, seq)`,
/// and paging resumes strictly after this tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cursor {
    pub db_version: i64,
    pub seq: i64,
}

impl Cursor {
    /// The zero cursor: pull everything from the beginning.
    pub const ZERO: Cursor = Cursor {
        db_version: 0,
        seq: 0,
    };

    /// Tuple ordering — strictly-after semantics for `(db_version, seq)`.
    pub fn is_strictly_after(&self, other: Cursor) -> bool {
        (self.db_version, self.seq) > (other.db_version, other.seq)
    }
}

/// One page of changes, the unit of one `Pull` → `Changes` exchange.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangesPage {
    pub changes: Vec<Change>,
    /// The cursor the next `Pull` resumes from.
    pub next: Cursor,
    /// `true` → more changes may exist; the client keeps paging.
    pub more: bool,
}

impl ChangesPage {
    /// An empty, terminal page: nothing changed, nothing more.
    pub fn empty(next: Cursor) -> Self {
        Self {
            changes: Vec::new(),
            next,
            more: false,
        }
    }
}

/// What a sync session against one peer accomplished, for the driver to
/// aggregate across the allowlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerSyncOutcome {
    /// Number of changes applied from this peer this session.
    pub applied: usize,
    /// The cursor this peer's changes were applied through.
    pub cursor: Cursor,
}

/// Errors a [`ChangeSource`] can produce. `Transient` failures (dial,
/// timeout, pool hiccup) are expected of an offline peer and never stop the
/// sync loop; `Fatal` failures mean the engine itself is misbehaving and must
/// stop it. Mirrors the engine crate's error kinds so the runtime retry
/// classifier carries over unchanged.
#[derive(Debug, thiserror::Error)]
pub enum ChangeSourceError {
    #[error("transient sync source failure: {0}")]
    Transient(String),
    #[error("fatal sync source failure: {0}")]
    Fatal(String),
}

impl ChangeSourceError {
    /// Whether this error must stop the sync loop (only `Fatal` does).
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }
}

/// The read/write surface the sync agent needs from the CRDT engine.
///
/// One implementation serves both session roles: as **server** (`pull`,
/// `record_served`) when a peer pulls from us, and as **client**
/// (`cursor_for`, `apply`) when we pull from a peer.
#[async_trait]
pub trait ChangeSource: Send + Sync + 'static {
    /// This site's cr-sqlite `site_id` (16 random bytes, learned from the
    /// engine on first open).
    async fn site_id(&self) -> Result<Vec<u8>, ChangeSourceError>;

    /// The highest applied `_sqlx_migrations.version`. A peer with a lower
    /// schema version is never served changes (it may lack the `cid`s).
    async fn schema_version(&self) -> Result<i64, ChangeSourceError>;

    /// The current cr-sqlite db version — the local write clock, reported to
    /// peers in `Hello` (diagnostics and a cheap liveness signal; the
    /// persisted cursor, not this, drives paging).
    async fn db_version(&self) -> Result<i64, ChangeSourceError>;

    /// Pull the next page of our changes strictly after `after`, excluding
    /// `exclude_site`'s own changes (a peer never needs its own writes back),
    /// within a serialized-byte budget of `max_bytes`. Always yields at least
    /// one row when one exists past the cursor (the budget can be exceeded
    /// by that single row, never silently truncated to zero).
    async fn pull(
        &self,
        after: Cursor,
        exclude_site: &[u8],
        max_bytes: usize,
    ) -> Result<ChangesPage, ChangeSourceError>;

    /// Apply a page received from `peer_site`, advancing the persisted cursor
    /// for that peer **in the same transaction** as the applied rows.
    /// cr-sqlite's apply is idempotent (a no-op for a losing
    /// `(col_version, site_id)`), so re-delivery is safe.
    async fn apply(&self, peer_site: &[u8], page: &ChangesPage) -> Result<(), ChangeSourceError>;

    /// The persisted cursor for `peer_site`: where its next pull resumes.
    async fn cursor_for(&self, peer_site: &[u8]) -> Result<Cursor, ChangeSourceError>;

    /// Record that we have served this peer through `through` — makes
    /// `has_unsent_changes` honest (`crsql_db_version() > min(served)`).
    async fn record_served(
        &self,
        peer_site: &[u8],
        through: Cursor,
    ) -> Result<(), ChangeSourceError>;
}

// ---------------------------------------------------------------------------
// FakeSource — the in-memory ChangeSource the crate's tests drive
// ---------------------------------------------------------------------------

/// An in-memory [`ChangeSource`]: the tests' stand-in for the real database.
///
/// Holds a set of changes indexed by `(db_version, seq)`, a per-peer cursor
/// map (standing in for `sync_peer_cursors`), and the two identity fields a
/// real source reads from the engine. `pull` implements the same
/// strictly-after + byte-budget + always-at-least-one-row contract the real
/// source must honor, so the session/transport tests exercise the paging
/// semantics the wire protocol will actually see.
///
/// Not `#[cfg(test)]`: PR slice C-F's convergence tests (integration tests
/// in `tests/`) and plugins/db's lifecycle tests need it too, and a unit-only
/// item would not be visible there.
#[derive(Debug, Default)]
pub struct FakeSource {
    inner: Mutex<FakeSourceInner>,
}

#[derive(Debug, Default)]
struct FakeSourceInner {
    site_id: Vec<u8>,
    schema_version: i64,
    /// The db version this source reports in `Hello` (FakeSource has no real
    /// write clock; the tests set it).
    db_version: i64,
    /// Ordered by `(db_version, seq)` — BTreeMap gives us the tuple paging
    /// for free.
    changes: BTreeMap<(i64, i64), Change>,
    /// Pulled cursors, keyed by peer site id (→ `cursor_for` / `apply`).
    cursors: BTreeMap<Vec<u8>, Cursor>,
    /// Served cursors, keyed by peer site id (→ `record_served`).
    served: BTreeMap<Vec<u8>, Cursor>,
    /// When set, every operation fails with this error — drives the
    /// offline/fatal-error paths deterministically.
    fail_with: Option<ChangeSourceError>,
}

impl FakeSource {
    /// A fresh source with an empty change log and no cursors.
    pub fn new() -> Self {
        Self::default()
    }

    /// A shared source, ready to hand to an agent or driver.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Set this source's site id (the tests use small distinct byte strings).
    pub async fn set_site_id(&self, site_id: &[u8]) {
        self.inner.lock().await.site_id = site_id.to_vec();
    }

    /// Set the schema version this source reports.
    pub async fn set_schema_version(&self, schema_version: i64) {
        self.inner.lock().await.schema_version = schema_version;
    }

    /// Set the db version this source reports in `Hello`.
    pub async fn set_db_version(&self, db_version: i64) {
        self.inner.lock().await.db_version = db_version;
    }

    /// Add one change to the change log, keyed by its own `(db_version, seq)`.
    pub async fn add_change(&self, change: Change) {
        let mut inner = self.inner.lock().await;
        inner
            .changes
            .insert((change.db_version, change.seq), change);
    }

    /// Convenience: append a text-valued change at the next free
    /// `(db_version, 1)` slot with the given origin `site_id`.
    pub async fn add_text_change(&self, db_version: i64, site_id: &[u8], table: &str, val: &str) {
        self.add_change(Change {
            table: table.to_string(),
            pk: SqlValue::Text(format!("pk-{db_version}")),
            cid: "value".to_string(),
            val: SqlValue::Text(val.to_string()),
            col_version: 1,
            db_version,
            site_id: site_id.to_vec(),
            cl: 1,
            seq: 1,
        })
        .await;
    }

    /// Make every operation on this source fail with `error` — the
    /// deterministic offline/fatal-error path.
    pub async fn fail_with(&self, error: Option<ChangeSourceError>) {
        self.inner.lock().await.fail_with = error;
    }

    /// The served cursor recorded for `peer_site` (test assertions).
    pub async fn served_cursor_for(&self, peer_site: &[u8]) -> Option<Cursor> {
        self.inner.lock().await.served.get(peer_site).copied()
    }

    /// The pull cursor recorded for `peer_site` (test assertions).
    pub async fn pull_cursor_for(&self, peer_site: &[u8]) -> Option<Cursor> {
        self.inner.lock().await.cursors.get(peer_site).copied()
    }
}

#[async_trait]
impl ChangeSource for FakeSource {
    async fn site_id(&self) -> Result<Vec<u8>, ChangeSourceError> {
        let inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        Ok(inner.site_id.clone())
    }

    async fn schema_version(&self) -> Result<i64, ChangeSourceError> {
        let inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        Ok(inner.schema_version)
    }

    async fn db_version(&self) -> Result<i64, ChangeSourceError> {
        let inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        Ok(inner.db_version)
    }

    async fn pull(
        &self,
        after: Cursor,
        exclude_site: &[u8],
        max_bytes: usize,
    ) -> Result<ChangesPage, ChangeSourceError> {
        let inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }

        let mut changes = Vec::new();
        let mut next = after;
        for ((db_version, seq), change) in inner.changes.range((
            std::ops::Bound::Excluded((after.db_version, after.seq)),
            std::ops::Bound::Unbounded,
        )) {
            if change.site_id == exclude_site {
                next = Cursor {
                    db_version: *db_version,
                    seq: *seq,
                };
                continue;
            }
            // Always include at least one row past the cursor, then honor the
            // byte budget — the contract the real source must keep.
            let cost = change_serialized_size(change);
            if !changes.is_empty()
                && changes
                    .iter()
                    .map(|c| change_serialized_size(c))
                    .sum::<usize>()
                    + cost
                    > max_bytes
            {
                return Ok(ChangesPage {
                    changes,
                    next,
                    more: true,
                });
            }
            changes.push(change.clone());
            next = Cursor {
                db_version: *db_version,
                seq: *seq,
            };
        }
        Ok(ChangesPage {
            changes,
            next,
            more: false,
        })
    }

    async fn apply(&self, peer_site: &[u8], page: &ChangesPage) -> Result<(), ChangeSourceError> {
        let mut inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        for change in &page.changes {
            inner
                .changes
                .insert((change.db_version, change.seq), change.clone());
        }
        inner.cursors.insert(peer_site.to_vec(), page.next);
        Ok(())
    }

    async fn cursor_for(&self, peer_site: &[u8]) -> Result<Cursor, ChangeSourceError> {
        let inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        Ok(inner
            .cursors
            .get(peer_site)
            .copied()
            .unwrap_or(Cursor::ZERO))
    }

    async fn record_served(
        &self,
        peer_site: &[u8],
        through: Cursor,
    ) -> Result<(), ChangeSourceError> {
        let mut inner = self.inner.lock().await;
        if let Some(err) = &inner.fail_with {
            return Err(clone_error(err));
        }
        inner.served.insert(peer_site.to_vec(), through);
        Ok(())
    }
}

/// `ChangeSourceError` is never constructed cheaply enough to `to_string`
/// round-trip reliably (thiserror messages are fine, but the inner lock is
/// held — clone by kind).
fn clone_error(err: &ChangeSourceError) -> ChangeSourceError {
    match err {
        ChangeSourceError::Transient(msg) => ChangeSourceError::Transient(msg.clone()),
        ChangeSourceError::Fatal(msg) => ChangeSourceError::Fatal(msg.clone()),
    }
}

/// Approximate serialized size of one change — good enough for the byte
/// budget the FakeSource simulates.
fn change_serialized_size(change: &Change) -> usize {
    change.table.len()
        + change.cid.len()
        + change.site_id.len()
        + 8 /* col_version */
        + 8 /* db_version */
        + 8 /* seq */
        + match &change.val {
            SqlValue::Null => 0,
            SqlValue::Integer(_) | SqlValue::Real(_) => 8,
            SqlValue::Text(s) => s.len(),
            SqlValue::Blob(b) => b.len(),
        }
        + match &change.pk {
            SqlValue::Text(s) => s.len(),
            SqlValue::Blob(b) => b.len(),
            _ => 8,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(db_version: i64, site: &[u8]) -> Change {
        Change {
            table: "sessions".into(),
            pk: SqlValue::Text(format!("pk-{db_version}")),
            cid: "title".into(),
            val: SqlValue::Text(format!("v{db_version}")),
            col_version: 1,
            db_version,
            site_id: site.to_vec(),
            cl: 1,
            seq: 1,
        }
    }

    /// The page contract: strictly-after ordering, exclusion of the puller's
    /// own site, always at least one row past the cursor, budget honored
    /// after the first row.
    #[tokio::test]
    async fn pull_is_strictly_after_and_excludes_self() {
        let source = FakeSource::new();
        let a = b"site-a".to_vec();
        let b = b"site-b".to_vec();
        for v in 1..=5 {
            source.add_change(change(v, &a)).await;
        }
        source.add_change(change(6, &b)).await;

        // Full pull excluding site-b returns a's five changes, while the
        // cursor advances past b's excluded tail row to prevent re-scanning it.
        let page = source.pull(Cursor::ZERO, &b, usize::MAX).await.unwrap();
        assert_eq!(page.changes.len(), 5);
        assert_eq!(
            page.next,
            Cursor {
                db_version: 6,
                seq: 1
            }
        );
        assert!(!page.more);

        // Resuming at `next` yields nothing more.
        let page2 = source.pull(page.next, &b, usize::MAX).await.unwrap();
        assert!(page2.changes.is_empty());
        assert!(!page2.more);

        // Pulling as site-a excludes a's own changes: only b's.
        let page3 = source.pull(Cursor::ZERO, &a, usize::MAX).await.unwrap();
        assert_eq!(page3.changes.len(), 1);
        assert_eq!(page3.changes[0].site_id, b);
    }

    /// The budget always yields the first row past the cursor, then stops.
    #[tokio::test]
    async fn pull_budget_never_yields_an_empty_first_page() {
        let source = FakeSource::new();
        let a = b"site-a".to_vec();
        for v in 1..=3 {
            source.add_change(change(v, &a)).await;
        }

        // A budget of 1 byte: first row comes anyway, more == true.
        let page = source.pull(Cursor::ZERO, b"site-b", 1).await.unwrap();
        assert_eq!(page.changes.len(), 1, "at least one row always");
        assert!(page.more, "budget hit → more");
        assert_eq!(page.next.db_version, 1);

        // Paging to exhaustion at budget 1: two more pages of one row each.
        let page2 = source.pull(page.next, b"site-b", 1).await.unwrap();
        assert_eq!(page2.changes.len(), 1);
        assert!(page2.more);
        let page3 = source.pull(page2.next, b"site-b", 1).await.unwrap();
        assert_eq!(page3.changes.len(), 1);
        assert!(!page3.more);
    }

    /// `apply` records the cursor atomically with the rows: a page whose
    /// apply completes has advanced `cursor_for` to `page.next` — re-pulling
    /// from there must never re-deliver the same rows.
    #[tokio::test]
    async fn apply_advances_the_cursor_with_the_rows() {
        let source = FakeSource::new();
        let peer = b"peer".to_vec();

        let page = ChangesPage {
            changes: vec![change(1, &peer), change(2, &peer)],
            next: Cursor {
                db_version: 2,
                seq: 1,
            },
            more: true,
        };
        source.apply(&peer, &page).await.unwrap();

        assert_eq!(
            source.cursor_for(&peer).await.unwrap(),
            Cursor {
                db_version: 2,
                seq: 1
            },
            "cursor advanced to page.next"
        );
        // The rows are visible through pull — idempotent re-apply is the
        // engine's business; the fake keeps the map keyed by (db_version, seq).
        let back = source
            .pull(Cursor::ZERO, b"other", usize::MAX)
            .await
            .unwrap();
        assert_eq!(back.changes.len(), 2);
    }

    /// `record_served` is separate from the pull cursor: the served map is
    /// what `has_unsent_changes` consults, the pull map is where the next
    /// pull resumes. Both must be keyed by peer site id.
    #[tokio::test]
    async fn served_cursor_is_tracked_per_peer() {
        let source = FakeSource::new();
        let peer_a = b"peer-a".to_vec();
        let peer_b = b"peer-b".to_vec();

        source
            .record_served(
                &peer_a,
                Cursor {
                    db_version: 9,
                    seq: 3,
                },
            )
            .await
            .unwrap();
        source
            .record_served(
                &peer_b,
                Cursor {
                    db_version: 1,
                    seq: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            source.served_cursor_for(&peer_a).await,
            Some(Cursor {
                db_version: 9,
                seq: 3
            })
        );
        assert_eq!(
            source.served_cursor_for(&peer_b).await,
            Some(Cursor {
                db_version: 1,
                seq: 0
            })
        );
    }

    /// A failing source fails every operation, and `is_fatal` classifies.
    #[tokio::test]
    async fn failure_injection_works() {
        let source = FakeSource::new();
        source
            .fail_with(Some(ChangeSourceError::Fatal("engine broken".into())))
            .await;

        let err = source.site_id().await.unwrap_err();
        assert!(err.is_fatal());
        assert!(source.pull(Cursor::ZERO, b"x", 10).await.is_err());
        source.fail_with(None).await;
        assert_eq!(source.site_id().await.unwrap(), Vec::<u8>::new());
    }
}
