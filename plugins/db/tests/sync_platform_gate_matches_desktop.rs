//! Guards against exactly the failure class docs/internal/sync-p2p.md §22
//! exists to avoid: nothing in Cargo itself enforces that this crate's and
//! the desktop crate's `[target.'cfg(...)'.dependencies]` tables stay
//! textually identical (Cargo resolves target-cfg dependency tables before
//! any build script runs, so they can't just reference the `sync_platform`
//! rustc-cfg both crates' Rust code is gated on). If the two tables drift,
//! the app can resolve `sync-p2p`/`hypr-cloudsync` on one platform set while
//! `sync_platform` (see build-support/sync_app_gate.rs) gates the actual
//! code on another — a divergent gate, silent until someone builds on the
//! mismatched platform. This test makes the drift a compile-time-adjacent
//! test failure instead.

const SYNC_PLATFORM_CFG: &str = r#"cfg(any(all(target_os = "linux", target_arch = "x86_64"), all(target_os = "macos", target_arch = "aarch64"), all(target_os = "macos", target_arch = "x86_64"), all(target_os = "windows", target_arch = "x86_64")))"#;

#[test]
fn plugins_db_and_desktop_cargo_toml_share_the_same_sync_platform_cfg() {
    let this_crate = include_str!("../Cargo.toml");
    let desktop_crate = include_str!("../../../apps/desktop/src-tauri/Cargo.toml");

    assert!(
        this_crate.contains(SYNC_PLATFORM_CFG),
        "plugins/db/Cargo.toml's sync-platform target table drifted from the \
         expected cfg expression (see build-support/sync_app_gate.rs)"
    );
    assert!(
        desktop_crate.contains(SYNC_PLATFORM_CFG),
        "apps/desktop/src-tauri/Cargo.toml's sync-platform target table \
         drifted from plugins/db/Cargo.toml's (see docs/internal/sync-p2p.md §22)"
    );
}
