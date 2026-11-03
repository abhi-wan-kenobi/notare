//! The `crsql_changes` surface: the value codec, the row shape, cursor
//! paging, pull and apply.
//!
//! This is the transport-facing module. The 0.7 P2P protocol (sync-p2p
//! protocol v2) ships [`Change`] and [`Cursor`] on the wire, so both are
//! `Serialize`/`Deserialize` (JSON; a CBOR pivot is a later optimization).
//!
//! cr-sqlite v0.16.3 `crsql_changes` columns (pinned by the Phase 0 spike,
//! sync-p2p.md §30): `"table", pk, cid, val, col_version, db_version,
//! site_id, cl, seq` — `cl` exists but is always 1 at this version; `ts`
//! does NOT exist (that is the superfly fork's schema change).

use serde::{Deserialize, Serialize};
use sqlx::query::Query;
use sqlx::{Executor, Row, Sqlite, TypeInfo, ValueRef};

use crate::error::Error;

/// The value codec the 0.7 transport ships. Decoding goes through
/// `SqliteValueRef::type_info()` — the *actual* storage class of the
/// underlying sqlite3_value, not the column's declared affinity — so the
/// transport never textualises or re-scores a value. Proven byte-exact for
/// every storage class including i64::MIN/MAX, -0.0, 1e300, embedded NULs
/// and 1 MiB blobs by the Phase 0 STRICT spike.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    /// Decode one SQLite value by its runtime storage class.
    ///
    /// # Panics
    /// On an unexpected storage class or on invalid UTF-8 in a TEXT value
    /// (the app schema guarantees TEXT columns hold UTF-8 — the same
    /// contract the spike's codec pinned).
    pub fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Self {
        use sqlx::Decode;

        let info = value.type_info();
        match info.name() {
            "NULL" => Self::Null,
            "INTEGER" => {
                Self::Integer(i64::decode(value).expect("decode INTEGER storage class as i64"))
            }
            "REAL" => Self::Real(f64::decode(value).expect("decode REAL storage class as f64")),
            "TEXT" => {
                Self::Text(String::decode(value).expect("decode TEXT storage class as String"))
            }
            "BLOB" => {
                Self::Blob(Vec::<u8>::decode(value).expect("decode BLOB storage class as Vec<u8>"))
            }
            other => panic!("unexpected storage class in crsql_changes.val: {other}"),
        }
    }

    /// Bind this value as the next query argument.
    pub fn bind<'q>(
        self,
        query: Query<'q, Sqlite, sqlx::sqlite::SqliteArguments>,
    ) -> Query<'q, Sqlite, sqlx::sqlite::SqliteArguments> {
        match self {
            Self::Null => query.bind(None::<i64>),
            Self::Integer(v) => query.bind(v),
            Self::Real(v) => query.bind(v),
            Self::Text(v) => query.bind(v),
            Self::Blob(v) => query.bind(v),
        }
    }

    /// The SQL `typeof()` string for this value, for cross-node typeof
    /// equality assertions (the STRICT question).
    pub fn typeof_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Integer(_) => "integer",
            Self::Real(_) => "real",
            Self::Text(_) => "text",
            Self::Blob(_) => "blob",
        }
    }
}

/// A `crsql_changes` row, as the transport ships it.
///
/// `pk` is cr-sqlite's packed primary key (a blob for composite PKs; a
/// single `SqlValue` for the single-column TEXT PKs the synced tables use).
/// `cid` is the column name, or `"-1"` for a row tombstone (hard DELETE).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

impl Change {
    /// Approximate encoded size of this change, for `max_bytes` paging.
    fn approx_size(&self) -> usize {
        const ROW_OVERHEAD: usize = 64;
        let val = match &self.val {
            SqlValue::Null => 0,
            SqlValue::Integer(_) | SqlValue::Real(_) => 8,
            SqlValue::Text(s) => s.len(),
            SqlValue::Blob(b) => b.len(),
        };
        let pk = match &self.pk {
            SqlValue::Null => 0,
            SqlValue::Integer(_) | SqlValue::Real(_) => 8,
            SqlValue::Text(s) => s.len(),
            SqlValue::Blob(b) => b.len(),
        };
        ROW_OVERHEAD + val + pk + self.table.len() + self.cid.len() + self.site_id.len()
    }
}

/// Position in the `crsql_changes` stream: `(db_version, seq)` is a total
/// order over changes on a node, so a cursor is resumable and duplicate-free
/// across pages — pull strictly after it.
///
/// `seq` restarts at 0 for each `db_version`, hence tuple comparison
/// `(db_version, seq) > (?, ?)`, not `db_version > ?`.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Cursor {
    pub db_version: i64,
    pub seq: i64,
}

impl Cursor {
    pub const START: Cursor = Cursor {
        db_version: 0,
        seq: 0,
    };
}

/// One page of changes from [`pull_changes`].
///
/// `next` is the cursor to resume from (the position just past the last
/// returned change); `more` is false when the stream is drained — a session
/// applies pages until `!more`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangesPage {
    pub changes: Vec<Change>,
    pub next: Cursor,
    pub more: bool,
}

/// The `crsql_changes` column list, as bound by [`pull_changes`] and
/// [`apply_changes`].
const CHANGES_COLUMNS: &str = r#""table", pk, cid, val, col_version, db_version, site_id, cl, seq"#;

/// Pull the next page of changes strictly after `after`, excluding
/// `exclude_site`'s own changes (the local site's — pulling from a peer must
/// never feed a node its own writes back).
///
/// Paging is by the `(db_version, seq)` tuple, so consecutive pages tile the
/// stream exactly: a change is returned by one page or the next, never both
/// and never neither. `max_bytes` is the page budget in approximate encoded
/// bytes (default and cap enforced by the caller; always ≥1 row — see the
/// 2026-09-07 plan's backpressure section); the receiver's apply speed
/// throttles the sender one page at a time.
pub async fn pull_changes<'e, E>(
    executor: E,
    after: Cursor,
    exclude_site: &[u8],
    max_bytes: usize,
) -> Result<ChangesPage, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = format!(
        r#"SELECT {CHANGES_COLUMNS}
           FROM crsql_changes
           WHERE (db_version, seq) > (?, ?) AND site_id IS NOT ?
           ORDER BY db_version, seq"#
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(after.db_version)
        .bind(after.seq)
        .bind(exclude_site)
        .fetch_all(executor)
        .await?;

    let mut changes: Vec<Change> = Vec::with_capacity(rows.len());
    let mut total = 0usize;
    let mut drained = true;

    for row in rows {
        let change = Change {
            table: row.try_get(0)?,
            pk: SqlValue::decode(row.try_get_raw(1).expect("raw pk")),
            cid: row.try_get(2)?,
            val: SqlValue::decode(row.try_get_raw(3).expect("raw val")),
            col_version: row.try_get(4)?,
            db_version: row.try_get(5)?,
            site_id: row.try_get(6)?,
            cl: row.try_get(7)?,
            seq: row.try_get(8)?,
        };

        if !changes.is_empty() && total + change.approx_size() > max_bytes {
            // Budget exhausted and at least one row already served — stop
            // before this change; the next page starts at it.
            drained = false;
            break;
        }

        total += change.approx_size();
        changes.push(change);
    }

    let next = if let Some(last) = changes.last() {
        Cursor {
            db_version: last.db_version,
            seq: last.seq,
        }
    } else {
        after
    };

    Ok(ChangesPage {
        changes,
        next,
        more: !drained,
    })
}

/// Apply a batch of changes via `INSERT INTO crsql_changes`.
///
/// Manages **no transaction of its own** (per the plan): the caller wraps
/// the apply and the peer-cursor advance in one `BEGIN; ...; COMMIT` on
/// this connection so the cursor never commits ahead of the data. From a
/// `Transaction`, pass `&mut *tx` (it derefs to the connection); from a
/// pool, acquire a connection first — the apply must land on one
/// connection, not round-robin the pool.
///
/// cr-sqlite's apply is a no-op for a losing `(col_version, site_id)` —
/// idempotency pinned by the spike. Passing an empty slice is a no-op.
pub async fn apply_changes(
    conn: &mut sqlx::sqlite::SqliteConnection,
    changes: &[Change],
) -> Result<(), Error> {
    use sqlx::Executor;

    let sql = format!(
        r#"INSERT INTO crsql_changes ({CHANGES_COLUMNS})
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
    );
    for change in changes {
        let query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(change.table.clone());
        let query = change.pk.clone().bind(query);
        let query = query.bind(change.cid.clone());
        let query = change.val.clone().bind(query);
        query
            .bind(change.col_version)
            .bind(change.db_version)
            .bind(change.site_id.clone())
            .bind(change.cl)
            .bind(change.seq)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The codec decodes every storage class from a plain SQL row without
    /// the extension — keeps the codec exercised on every CI run even where
    /// the engine tests are gated. Carried from the spike.
    #[tokio::test]
    async fn codec_covers_every_storage_class() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .unwrap();

        sqlx::query("CREATE TABLE v (n, i, r, t, b)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO v VALUES (NULL, 7, 0.25, 'x', X'00ff')")
            .execute(&pool)
            .await
            .unwrap();

        let row = sqlx::query("SELECT n, i, r, t, b FROM v")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            SqlValue::decode(row.try_get_raw(0).unwrap()),
            SqlValue::Null
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(1).unwrap()),
            SqlValue::Integer(7)
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(2).unwrap()),
            SqlValue::Real(0.25)
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(3).unwrap()),
            SqlValue::Text("x".into())
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(4).unwrap()),
            SqlValue::Blob(vec![0x00, 0xff])
        );

        pool.close().await;
    }

    /// `Change` and `Cursor` round-trip through serde — they are the P2P
    /// wire payload (protocol v2), so the derives must actually exist and
    /// produce the field names the peer expects.
    #[test]
    fn change_and_cursor_round_trip_through_serde() {
        let change = Change {
            table: "sessions".into(),
            pk: SqlValue::Text("sess-1".into()),
            cid: "title".into(),
            val: SqlValue::Text("hello".into()),
            col_version: 1,
            db_version: 3,
            site_id: vec![1u8; 16],
            cl: 1,
            seq: 2,
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains(r#""db_version":3"#));
        assert!(json.contains(r#""col_version":1"#));
        assert_eq!(serde_json::from_str::<Change>(&json).unwrap(), change);

        let cursor = Cursor {
            db_version: 3,
            seq: 7,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        assert_eq!(json, r#"{"db_version":3,"seq":7}"#);

        let page = ChangesPage {
            changes: vec![change],
            next: cursor,
            more: true,
        };
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains(r#""more":true"#));
        assert_eq!(serde_json::from_str::<ChangesPage>(&json).unwrap(), page);
    }

    /// Tuple paging must be a strict total-order comparison: `seq` restarts
    /// at 0 for each new `db_version`, so `(3, 0)` is *after* `(2, 99)`.
    /// This pins the ordering that makes pages tile the stream exactly.
    #[test]
    fn cursor_tuple_order_wraps_seq_per_db_version() {
        assert!(
            Cursor {
                db_version: 3,
                seq: 0
            } > Cursor {
                db_version: 2,
                seq: 99
            }
        );
        assert!(
            Cursor {
                db_version: 3,
                seq: 5
            } > Cursor {
                db_version: 3,
                seq: 4
            }
        );
        assert!(
            Cursor {
                db_version: 3,
                seq: 5
            } > Cursor::START
        );
        assert_eq!(Cursor::default(), Cursor::START);
    }
}
