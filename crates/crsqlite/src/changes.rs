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
    /// Fails (rather than panics) on an unexpected storage class or invalid
    /// UTF-8 in a TEXT value: `val` can carry a peer's write applied
    /// through `apply_changes`, and a remote `crsql_changes` row is not
    /// trusted input — a malformed or malicious row must error the sync
    /// session, not crash the process.
    pub fn decode(value: sqlx::sqlite::SqliteValueRef<'_>) -> Result<Self, Error> {
        use sqlx::Decode;

        let info = value.type_info();
        let name = info.name();
        let decoded = match name {
            "NULL" => Self::Null,
            "INTEGER" => Self::Integer(
                i64::decode(value).map_err(|e| Error::Decode(format!("INTEGER: {e}")))?,
            ),
            "REAL" => {
                Self::Real(f64::decode(value).map_err(|e| Error::Decode(format!("REAL: {e}")))?)
            }
            "TEXT" => {
                Self::Text(String::decode(value).map_err(|e| Error::Decode(format!("TEXT: {e}")))?)
            }
            "BLOB" => Self::Blob(
                Vec::<u8>::decode(value).map_err(|e| Error::Decode(format!("BLOB: {e}")))?,
            ),
            other => return Err(Error::Decode(format!("unexpected storage class {other:?}"))),
        };
        Ok(decoded)
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
/// `pk` is cr-sqlite's packed primary key — a BLOB at v0.16.3 even for the
/// single-column TEXT PKs the synced tables use; treat it as opaque.
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

/// Hard ceiling on [`pull_changes`]'s `limit`, independent of the caller's
/// request. Keeps `i64::try_from(limit + 1)` for the `LIMIT ?` probe row
/// (see [`pull_changes`]) well inside `i64` with room to spare, regardless
/// of what a caller passes.
const MAX_PULL_LIMIT: usize = 100_000;

/// Pull the next page of changes strictly after `after`, excluding
/// `exclude_site`'s own changes when given (the requesting peer's — serving
/// a peer must never feed it its own writes back).
///
/// Paging is by the `(db_version, seq)` tuple, so consecutive pages tile the
/// stream exactly: a change is returned by one page or the next, never both
/// and never neither. `limit` caps the page in rows, clamped to
/// `1..=MAX_PULL_LIMIT` — the sync-p2p session layer already clamps the
/// page size it asks for to its own `max_page_size` (5000 by default)
/// before calling this, so the ceiling here is defense in depth for other
/// callers, not a live path. `more` is exact — one extra row is probed, so
/// a page that ends precisely at the stream's tail reports `more == false`.
pub async fn pull_changes<'e, E>(
    executor: E,
    after: Cursor,
    exclude_site: Option<&[u8]>,
    limit: usize,
) -> Result<ChangesPage, Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let limit = limit.clamp(1, MAX_PULL_LIMIT);
    let probe = i64::try_from(limit + 1).expect("MAX_PULL_LIMIT + 1 fits in i64");
    let sql = format!(
        r#"SELECT {CHANGES_COLUMNS}
           FROM crsql_changes
           WHERE (db_version, seq) > (?, ?) AND (? IS NULL OR site_id IS NOT ?)
           ORDER BY db_version, seq
           LIMIT ?"#
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(after.db_version)
        .bind(after.seq)
        .bind(exclude_site)
        .bind(exclude_site)
        .bind(probe)
        .fetch_all(executor)
        .await?;

    let more = rows.len() > limit;
    let mut changes: Vec<Change> = Vec::with_capacity(rows.len().min(limit));
    for row in rows.into_iter().take(limit) {
        changes.push(Change {
            table: row.try_get(0)?,
            pk: SqlValue::decode(row.try_get_raw(1)?)?,
            cid: row.try_get(2)?,
            val: SqlValue::decode(row.try_get_raw(3)?)?,
            col_version: row.try_get(4)?,
            db_version: row.try_get(5)?,
            site_id: row.try_get(6)?,
            cl: row.try_get(7)?,
            seq: row.try_get(8)?,
        });
    }

    let next = changes.last().map_or(after, |last| Cursor {
        db_version: last.db_version,
        seq: last.seq,
    });

    Ok(ChangesPage {
        changes,
        next,
        more,
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
            SqlValue::decode(row.try_get_raw(0).unwrap()).unwrap(),
            SqlValue::Null
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(1).unwrap()).unwrap(),
            SqlValue::Integer(7)
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(2).unwrap()).unwrap(),
            SqlValue::Real(0.25)
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(3).unwrap()).unwrap(),
            SqlValue::Text("x".into())
        );
        assert_eq!(
            SqlValue::decode(row.try_get_raw(4).unwrap()).unwrap(),
            SqlValue::Blob(vec![0x00, 0xff])
        );

        pool.close().await;
    }

    /// cr-sqlite synced data comes from other devices via `apply_changes`,
    /// so a `crsql_changes.val` with SQLite TEXT storage class but non-UTF8
    /// bytes (SQLite itself does not enforce UTF-8 in TEXT) must error, not
    /// crash `pull_changes`'s decode of the sync apply path.
    #[tokio::test]
    async fn decode_errors_instead_of_panicking_on_invalid_utf8_text() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .unwrap();

        // CAST(x'ff' AS TEXT) gives SQLite TEXT storage class holding the
        // single byte 0xff, which is not valid UTF-8 on its own.
        let row = sqlx::query("SELECT CAST(x'ff' AS TEXT)")
            .fetch_one(&pool)
            .await
            .unwrap();

        let result = SqlValue::decode(row.try_get_raw(0).unwrap());
        assert!(
            matches!(result, Err(Error::Decode(_))),
            "expected a Decode error, got {result:?}"
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
