//! Teardown regression: a cr-sqlite pool must close without sqlx's worker
//! threads panicking on `sqlite3_close` ("unable to close due to
//! unfinalized statements"). Those panics happen on sqlx's own threads, so
//! they never fail a test on their own — a process-wide panic hook counts
//! them instead. One test per binary keeps the hook's count unpolluted.

#![cfg(all(
    feature = "crsqlite",
    any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )
))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use db_core::{Db, DbOpenOptions, DbStorage};

fn install_worker_panic_counter() -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let hook_count = Arc::clone(&count);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let on_sqlx_worker = std::thread::current()
            .name()
            .is_some_and(|name| name.starts_with("sqlx-sqlite-worker"));
        if on_sqlx_worker {
            hook_count.fetch_add(1, Ordering::SeqCst);
        }
        previous(info);
    }));
    count
}

async fn open(path: &std::path::Path) -> Db {
    Db::open_crsqlite(DbOpenOptions {
        storage: DbStorage::Local(path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap()
}

/// Touch the CRR table from four connections held at once, so every pooled
/// connection carries cr-sqlite statement-cache state.
async fn write_on_every_connection(db: &Db, round: usize) {
    let mut held = Vec::new();
    for i in 0..4 {
        let mut conn = db.pool().acquire().await.unwrap();
        sqlx::query("INSERT INTO notes (id, body) VALUES (?, ?)")
            .bind(format!("r{round}-c{i}"))
            .bind("x")
            .execute(&mut *conn)
            .await
            .unwrap();
        held.push(conn);
    }
    assert_eq!(db.pool().size(), 4);
}

#[tokio::test]
async fn crsqlite_pool_closes_without_worker_panics() {
    let panics = install_worker_panic_counter();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");

    {
        let db = open(&path).await;
        sqlx::query("CREATE TABLE notes (id TEXT PRIMARY KEY NOT NULL, body TEXT) STRICT")
            .execute(db.pool())
            .await
            .unwrap();
        hypr_crsqlite::as_crr(db.pool(), "notes").await.unwrap();
        db.close().await;
    }

    for round in 0..5 {
        let db = open(&path).await;
        write_on_every_connection(&db, round).await;
        db.close().await;
        assert!(db.pool().is_closed());
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        panics.load(Ordering::SeqCst),
        0,
        "Db::close must finalize every connection before sqlite3_close"
    );

    // Control: the same workload closed with a bare pool.close() does
    // panic the workers — proves the counter can see the failure mode.
    let db = open(&path).await;
    write_on_every_connection(&db, 99).await;
    db.pool().close().await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        panics.load(Ordering::SeqCst) > 0,
        "control: an unfinalized close is expected to panic sqlx workers"
    );

    // Every write from the five finalized rounds persisted.
    let plain = Db::connect_local_plain(&path).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes WHERE id NOT LIKE 'r99-%'")
        .fetch_one(plain.pool())
        .await
        .unwrap();
    assert_eq!(rows, 20);
    plain.close().await;
}
