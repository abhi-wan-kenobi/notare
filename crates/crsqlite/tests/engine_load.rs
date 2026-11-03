//! Un-gated engine test: loads the **vendored** cr-sqlite binary (extracted
//! from `include_bytes!` to the cache dir) on a plain sqlx pool and proves
//! the engine surface — the same probe `crsqlite_gate_linux` runs.
//!
//! This is the packaging 1a contract end-to-end: `apply()` must extract the
//! embedded binary for the current target, load it via
//! `SqliteConnectOptions::extension()` (the style already proven by
//! `hypr_cloudsync::apply`), and the extension must answer its API.
//!
//! Runs on every supported target with no environment variable — the gate
//! the `crsqlite_gate_{linux,macos,windows}` CI jobs execute on real
//! runners. On targets without a vendored binary (compile-time cfg),
//! `bundled_extension_path()` errors and this test cannot be compiled into
//! the binary at all; the whole file is cfg'd out there.

#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[tokio::test]
async fn loads_vendored_crsqlite_and_answers_the_api() {
    let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let (options, extension_path) = crsqlite::apply(options).unwrap();

    // The extraction must land in the cache dir under the pinned version
    // and this target's directory.
    assert!(
        extension_path.is_file(),
        "extracted extension must exist at {}",
        extension_path.display()
    );
    let path_str = extension_path.to_string_lossy();
    assert!(
        path_str.contains(crsqlite::CRSQLITE_VERSION),
        "extraction path must be versioned with CRSQLITE_VERSION ({}), got {path_str}",
        crsqlite::CRSQLITE_VERSION
    );

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    // Load probe: crsql_site_id() resolves only with the engine loaded.
    assert!(crsqlite::load_probe(&pool).await.unwrap());
    let site_id = crsqlite::site_id(&pool).await.unwrap();
    assert_eq!(site_id.len(), 16, "site id is 16 bytes");

    // db_version starts at 0 on a fresh database.
    assert_eq!(crsqlite::db_version(&pool).await.unwrap(), 0);

    // as_crr / is_crr on a fresh table.
    sqlx::query("CREATE TABLE probe (id TEXT PRIMARY KEY NOT NULL, v TEXT) STRICT")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!crsqlite::is_crr(&pool, "probe").await.unwrap());
    crsqlite::as_crr(&pool, "probe").await.unwrap();
    assert!(crsqlite::is_crr(&pool, "probe").await.unwrap());
    // Idempotent (Phase 0 finding, pinned here on the vendored binary).
    crsqlite::as_crr(&pool, "probe").await.unwrap();
    assert!(crsqlite::is_crr(&pool, "probe").await.unwrap());

    // A write bumps db_version and produces a change row.
    sqlx::query("INSERT INTO probe (id, v) VALUES ('p1', 'hello')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(crsqlite::db_version(&pool).await.unwrap() > 0);

    let page = crsqlite::pull_changes(
        &pool,
        crsqlite::Cursor::START,
        &[0u8; 16], // no real site can match all-zero: nothing excluded
        1024 * 1024,
    )
    .await
    .unwrap();
    assert!(!page.changes.is_empty(), "insert must produce changes");
    assert!((page.more == (page.next > crsqlite::Cursor::START)) || !page.more);

    // begin/commit_alter round-trip on the crr table.
    crsqlite::begin_alter(&pool, "probe").await.unwrap();
    crsqlite::commit_alter(&pool, "probe").await.unwrap();

    // finalize before close — the pool policy the close probe pinned.
    crsqlite::finalize(&pool).await.unwrap();

    pool.close().await;
}

/// A second `apply()` must not fail and must return the same path (the
/// extraction is idempotent — an existing file of the same length is left
/// alone).
#[test]
fn bundled_extension_path_is_stable_across_calls() {
    let a = crsqlite::bundled_extension_path().unwrap();
    let b = crsqlite::bundled_extension_path().unwrap();
    assert_eq!(a, b);
    assert!(a.is_file());
}
