//! Regression for the audit finding "`Db` drop without `close()` when
//! cr-sqlite is loaded": dropping a `Db` (forgetting `close()`, or
//! unwinding before reaching it) must not panic sqlx's worker thread on
//! `sqlite3_close`. `Drop` best-effort finalizes on a spawned task — this
//! proves that happens before this process sees an unfinalized-statements
//! panic.
//!
//! Kept in its own binary, same as `crsqlite_close.rs`: the panic-counting
//! hook below is process-global, so two tests racing on it in one binary
//! pollute each other's count.

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

#[tokio::test]
async fn dropping_a_crsqlite_db_without_close_does_not_panic_sqlx_workers() {
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

    let db = open(&path).await;
    // Touch every pooled connection so each carries cr-sqlite statement-cache
    // state, then drop the `Db` without calling `close()` — the bug this
    // regression test pins.
    let mut held = Vec::new();
    for i in 0..4 {
        let mut conn = db.pool().acquire().await.unwrap();
        sqlx::query("INSERT INTO notes (id, body) VALUES (?, ?)")
            .bind(format!("c{i}"))
            .bind("x")
            .execute(&mut *conn)
            .await
            .unwrap();
        held.push(conn);
    }
    drop(held);
    drop(db);

    // `Drop`'s best-effort finalize runs on a spawned tokio task; give it a
    // beat to run before checking the panic hook.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        panics.load(Ordering::SeqCst),
        0,
        "dropping a cr-sqlite Db must finalize connections before sqlite3_close, \
         not just Db::close"
    );
}
