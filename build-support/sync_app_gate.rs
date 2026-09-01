// Single source of truth for which (target_os, target_arch) pairs the
// desktop app's P2P sync stack (`sync-p2p` + `hypr-cloudsync`, gated behind
// `tauri-plugin-db`'s `sync` feature) is wired into.
//
// This is DELIBERATELY narrower than `crates/cloudsync/build.rs`'s
// `supported()`: that function says the vendored C `from-source` transport
// compiles and links; this file says CI has actually observed the *app*
// crate (`cargo check -p desktop --features sync`) compile on that target.
// See docs/internal/sync-p2p.md §21.8 and §22.
//
// linux/aarch64 is admitted by `supported()` but excluded here: as of this
// writing no CI job and no local toolchain have ever compiled anything
// against it (§21.7). Widen this file (and the two Cargo.toml `cfg(...)`
// tables that mirror it — `plugins/db/Cargo.toml` and
// `apps/desktop/src-tauri/Cargo.toml`) only once that changes.
//
// Included verbatim (via `include!`) by the `build.rs` of both
// `plugins/db` and `apps/desktop/src-tauri`, so there is exactly ONE copy of
// this predicate, not several hand-synced ones. `[target.'cfg(...)'.dependencies]`
// tables in Cargo.toml cannot reference a custom `#[cfg]` emitted by a build
// script (Cargo resolves dependencies before any build script runs), so
// those two tables necessarily spell the same boolean out by hand in TOML —
// keep them textually identical to each other and semantically identical to
// the `matches!` below.

fn sync_app_gate_supported() -> bool {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    matches!(
        (os.as_str(), arch.as_str()),
        ("linux", "x86_64") | ("macos", "aarch64") | ("macos", "x86_64") | ("windows", "x86_64")
    )
}

/// Emits the `sync_platform` cfg for the current crate when the target is
/// one this app gate admits, plus the `rustc-check-cfg` declaration so
/// `#[cfg(sync_platform)]` never trips `unexpected_cfgs`. Call once from
/// `main()` in each including `build.rs`.
fn emit_sync_app_gate_cfg() {
    println!("cargo::rustc-check-cfg=cfg(sync_platform)");
    if sync_app_gate_supported() {
        println!("cargo:rustc-cfg=sync_platform");
    }
}
