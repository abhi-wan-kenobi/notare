//! Phase 0 close-behaviour probe (PR slice C-A) — finding 1 from the
//! 2026-09-07 plan: **sqlx panics if `sqlite3_close` fails**
//! (`sqlx-sqlite/src/connection/handle.rs:136-145`).
//!
//! cr-sqlite caches prepared statements per connection and documents
//! `SELECT crsql_finalize()` before close. With a pool that recycles idle
//! connections, an unfinalised connection would take down the sqlx worker
//! thread (a panic there aborts the process — it is not catchable from the
//! test), so this must be settled before any pooling policy is written into
//! db-core (PR C-D's `idle_timeout(None).max_lifetime(None)` +
//! `sync_finalize_all` decision).
//!
//! Gating: same as `strict_roundtrip.rs` — `CRSQLITE_SPIKE_EXTENSION` unset
//! → print "skipped" and return.
//!
//! Scenarios (from the plan, verbatim):
//!
//! 1. 4-connection pool, `idle_timeout(100ms)`, touch crr tables, sleep,
//!    `pool.close()` — record panic or not.
//! 2. Same, but `crsql_finalize()` on each connection before drop.
//! 3. `idle_timeout(None).max_lifetime(None)` + a finalize-all sweep
//!    (the candidate pool policy).
//!
//! Whichever survives becomes the pool policy; the measured outcome is
//! recorded in `docs/internal/sync-p2p.md` §30.
//!
//! Process-safety note: a worker-thread panic aborts the test binary, so
//! each scenario runs on its own OS thread with its own tokio runtime and
//! reports through a channel. A scenario that aborts the process prints its
//! banner line to stdout first, so the run log still shows which scenario
//! died. The outcome lines are assertions of record: `SURVIVED` must be
//! reached for the policy the plan proposes (scenario 3); scenarios 1–2 are
//! measurements whose outcome feeds the §30 write-up either way.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crsqlite::spike::{connect_options, Node};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

fn extension_path() -> Option<PathBuf> {
    crsqlite::spike::extension_path_from_env()
}

fn skip_notice(test: &str) {
    println!("skipped ({test}): CRSQLITE_SPIKE_EXTENSION is not set");
}

/// Touch the crr tables on every connection the scenario wants exercised:
/// one insert per acquired connection, so each pooled connection has cr-sqlite
/// prepared-statement cache state, then released back to the idle queue.
async fn touch_crr_tables(pool: &SqlitePool, tag: &str, connections: u32) {
    for i in 0..connections {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("INSERT INTO sessions (id, title) VALUES (?, ?)")
            .bind(format!("{tag}-{i}"))
            .bind(tag)
            .execute(&mut *conn)
            .await
            .unwrap();
        drop(conn);
    }
}

/// `SELECT crsql_finalize()` on every currently-open connection. With a
/// pool, idle connections sit in the idle queue where we cannot reach them,
/// so the sweep acquires (and holds) each connection in turn up to
/// `max_connections` and finalizes it. This is the shape the plan's
/// `Db::sync_finalize_all()` would take.
async fn finalize_all(pool: &SqlitePool, connections: u32) {
    let mut held = Vec::new();
    for _ in 0..connections {
        held.push(pool.acquire().await.unwrap());
    }
    for conn in &mut held {
        sqlx::query("SELECT crsql_finalize()")
            .execute(&mut **conn)
            .await
            .unwrap();
    }
}

/// Migrate + as_crr + touch + (optional finalize sweep) + close under the
/// scenario's pool policy, reporting each stage through the channel. A
/// worker-thread abort during `pool.close()` kills the process before the
/// `SURVIVED` line is sent — which is exactly the finding it would be.
async fn scenario_body(
    extension: &Path,
    label: &'static str,
    pool_options: SqlitePoolOptions,
    finalize: bool,
    tx: &mpsc::Sender<String>,
) {
    let node = Node::open(label, extension, 4)
        .await
        .unwrap_or_else(|e| panic!("open {label}: {e}"));
    node.as_crr("sessions").await.unwrap();

    // Node::open's pool used default options; the scenario needs its own
    // policy, so close it and re-open on the same file — the schema and the
    // `sessions` clock table persist in the file.
    node.pool.close().await;
    let db_path = node.dir.path().join(format!("{label}.db"));
    let options = connect_options(&db_path, extension);
    let pool = pool_options.connect_with(options).await.unwrap();

    touch_crr_tables(&pool, label, 4).await;

    // Let idle_timeout (scenario 1–2: 100 ms) recycle connections before
    // close; 300 ms covers several recycles.
    tokio::time::sleep(Duration::from_millis(300)).await;

    if finalize {
        finalize_all(&pool, 4).await;
        let _ = tx.send(format!("{label}: finalize-all sweep completed"));
    }

    println!("[close-probe] {label}: calling pool.close()");
    pool.close().await;
    let _ = tx.send(format!("{label}: pool.close() returned without panic"));
    let _ = tx.send(format!("{label}: SURVIVED"));
}

/// Drive one scenario on its own thread + current-thread runtime. Panics on
/// the scenario thread are caught and reported; a panic inside a sqlx
/// worker thread aborts the process (the banner line already printed says
/// which scenario it was).
fn run_on_scenario_thread(
    banner: &str,
    label: &'static str,
    idle_timeout: Option<Duration>,
    finalize: bool,
) {
    let (tx, rx) = mpsc::channel();
    let banner = banner.to_string();

    let handle = std::thread::spawn(move || {
        println!("[close-probe] {banner}");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("scenario runtime");
        let extension = extension_path().expect("extension checked by caller");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.block_on(async {
                let mut options = SqlitePoolOptions::new()
                    .max_connections(4)
                    .idle_timeout(idle_timeout);
                if idle_timeout.is_none() {
                    // The candidate policy: no recycling at all.
                    options = options.max_lifetime(None);
                }
                scenario_body(&extension, label, options, finalize, &tx).await;
            });
        }));
        if let Err(panic) = result {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_string());
            let _ = tx.send(format!("{label}: PANIC on scenario thread: {message}"));
        }
    });
    handle.join().expect("scenario thread must not panic");

    let mut survived = false;
    for message in rx.try_iter() {
        println!("[close-probe] {message}");
        if message.ends_with("SURVIVED") {
            survived = true;
        }
    }
    assert!(
        survived,
        "scenario {label} must reach SURVIVED — if the process did not abort, \
         the close path still failed; see the [close-probe] lines above"
    );
}

/// Scenario 1: 4-connection pool, `idle_timeout(100ms)`, touch, sleep,
/// `pool.close()` — no finalize. Record panic or not.
#[test]
fn close_with_idle_timeout_and_no_finalize() {
    if extension_path().is_none() {
        skip_notice("close_with_idle_timeout_and_no_finalize");
        return;
    }
    run_on_scenario_thread(
        "scenario 1: idle_timeout(100ms), no finalize",
        "close1",
        Some(Duration::from_millis(100)),
        false,
    );
}

/// Scenario 2: same pool policy, but `crsql_finalize()` on each connection
/// before close.
#[test]
fn close_with_idle_timeout_and_finalize() {
    if extension_path().is_none() {
        skip_notice("close_with_idle_timeout_and_finalize");
        return;
    }
    run_on_scenario_thread(
        "scenario 2: idle_timeout(100ms) + crsql_finalize() sweep",
        "close2",
        Some(Duration::from_millis(100)),
        true,
    );
}

/// Scenario 3: the candidate pool policy — `idle_timeout(None)`,
/// `max_lifetime(None)` + a finalize-all sweep before close. This is the
/// policy PR C-D writes into db-core, so it is asserted, not just recorded.
#[test]
fn close_with_no_recycling_and_finalize_sweep() {
    if extension_path().is_none() {
        skip_notice("close_with_no_recycling_and_finalize_sweep");
        return;
    }
    run_on_scenario_thread(
        "scenario 3: idle_timeout(None).max_lifetime(None) + finalize sweep",
        "close3",
        None,
        true,
    );
}