use sqlx::SqlitePool;

use crate::CLOUDSYNC_MANAGED_DB_ID;
use crate::error::Error;

async fn query_with_optional_params(
    pool: &SqlitePool,
    fn_name: &str,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    Ok(match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}()")))
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?)")))
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(NULL, ?)")))
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?, ?)")))
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    })
}

/// Build the SQL for a parameterless-or-parameterized cloudsync network call,
/// returning the raw JSON TEXT the extension emits. The C functions return
/// JSON (`{"receive":{"rows":N,...}}`), not an integer — `query_scalar::<_,
/// i64>` would coerce the leading `{` to 0 via `sqlite3_value_int64`, which is
/// exactly why a drain loop cannot use the i64 return to decide "anything
/// pending?". Fetch the string and parse `receive.rows` instead (same shape as
/// `rows_received` in `sync_three_nodes.rs`).
async fn query_sync_json(
    pool: &SqlitePool,
    fn_name: &str,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<String, Error> {
    let row: (String,) = match (wait_ms, max_retries) {
        (None, None) => {
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {fn_name}() AS it")))
                .fetch_one(pool)
                .await?
        }
        (Some(wait_ms), None) => {
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?) AS it")))
                .bind(wait_ms)
                .fetch_one(pool)
                .await?
        }
        (None, Some(max_retries)) => {
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {fn_name}(NULL, ?) AS it"
            )))
            .bind(max_retries)
            .fetch_one(pool)
            .await?
        }
        (Some(wait_ms), Some(max_retries)) => {
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {fn_name}(?, ?) AS it")))
                .bind(wait_ms)
                .bind(max_retries)
                .fetch_one(pool)
                .await?
        }
    };
    Ok(row.0)
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-init
///
/// SYNC-5: uses the 2-arg `_custom` form. The 1-arg
/// `cloudsync_network_init(managedDatabaseId)` binds its argument as the
/// **db id** and hardcodes the address to the sqlitecloud SaaS
/// (`CLOUDSYNC_DEFAULT_ADDRESS`) — so a `p2p://<fingerprint>` connection
/// string bound via the 1-arg form never reaches the endpoint builder and
/// every request routes at the SaaS address instead of the addressed peer.
/// The convergence proofs (`crates/sync-p2p/examples/`, `drain_regression`)
/// all use `_custom` for exactly this reason.
pub async fn network_init(pool: &SqlitePool, connection_string: &str) -> Result<(), Error> {
    // SYNC-5: the db id namespaces this database's blobs on the hub. The
    // proofs use the app name; we use the same fixed id so a notare hub
    // recognises blobs from every device of the same app.
    sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
        .bind(connection_string)
        .bind(CLOUDSYNC_MANAGED_DB_ID)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-apikey
pub async fn network_set_apikey(pool: &SqlitePool, api_key: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_apikey(?)")
        .bind(api_key)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-set-token
pub async fn network_set_token(pool: &SqlitePool, token: &str) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_set_token(?)")
        .bind(token)
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-cleanup
pub async fn network_cleanup(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_cleanup()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-has-unsent-changes
pub async fn network_has_unsent_changes(pool: &SqlitePool) -> Result<bool, Error> {
    Ok(
        sqlx::query_scalar("SELECT cloudsync_network_has_unsent_changes()")
            .fetch_one(pool)
            .await?,
    )
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-send-changes
pub async fn network_send_changes(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    query_with_optional_params(pool, "cloudsync_network_send_changes", wait_ms, max_retries).await
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-check-changes
pub async fn network_check_changes(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    query_with_optional_params(
        pool,
        "cloudsync_network_check_changes",
        wait_ms,
        max_retries,
    )
    .await
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-reset-sync-version
pub async fn network_reset_sync_version(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_reset_sync_version()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-logout
pub async fn network_logout(pool: &SqlitePool) -> Result<(), Error> {
    sqlx::query("SELECT cloudsync_network_logout()")
        .fetch_optional(pool)
        .await?;

    Ok(())
}

/// Upper bound on drain rounds. The hub serves one blob per `check`; a site
/// that fell behind needs one round per pending blob. Without a bound a
/// non-converging hub would spin forever — so we cap and then fail loudly
/// rather than returning a misleading "synced".
const MAX_DRAIN: usize = 64;

/// Pull `receive.rows` out of a `network_sync` / `network_check_changes` JSON
/// reply. Returns `None` for an unreadable reply (deliberately NOT folded into
/// `Some(0)` — see [`Error::UnreadableSyncReply`]).
fn rows_received(resp: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    v.get("receive")?.get("rows")?.as_u64()
}

/// https://docs.sqlitecloud.io/docs/sqlite-sync-api-cloudsync-network-sync
///
/// **Drains.** The C `cloudsync_network_sync` does a send then loops
/// `check_internal` up to `max_retries`, **breaking on `nrows > 0`** — so a
/// single call pulls at most one pending blob (SYNC-4 divergence class: a
/// caller that syncs once silently stays behind). This wrapper loops the
/// whole send+check until the hub reports `receive.rows == 0`, bounded by
/// [`MAX_DRAIN`] and fail-loud on an unreadable reply. The total rows received
/// across all rounds is returned.
pub async fn network_sync(
    pool: &SqlitePool,
    wait_ms: Option<i64>,
    max_retries: Option<i64>,
) -> Result<i64, Error> {
    let mut total: i64 = 0;
    for _ in 0..MAX_DRAIN {
        let resp = query_sync_json(pool, "cloudsync_network_sync", wait_ms, max_retries).await?;
        match rows_received(&resp) {
            None => return Err(Error::UnreadableSyncReply(resp)),
            Some(0) => return Ok(total),
            Some(n) => total += n as i64,
        }
    }
    Err(Error::DrainExhausted(MAX_DRAIN))
}
