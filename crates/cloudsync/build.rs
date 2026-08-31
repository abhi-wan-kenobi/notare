// Builds the vendored sqlite-sync (CloudSync) extension from source into a
// loadable shared object, gated behind the `from-source` cargo feature.
//
// When the feature is OFF (the default), this script is a no-op and the crate
// keeps using the prebuilt `include_bytes!` artifacts in `vendor/cloudsync/`.
//
// Scope (SYNC-9): linux x86_64/aarch64, macOS aarch64/x86_64, Windows x86_64.
// Android is NOT supported here — it needs `vendor/src/network/cacert.h`
// (an Android-only PEM bundle wired up separately) and stays out of
// `supported()`; see the panic message and docs/internal/sync-p2p.md §21.
//
// On any other target this PANICS rather than quietly falling back, because it
// cannot fall back: the prebuilt `include_bytes!` artifacts in `bundle.rs` are
// `cfg(not(feature = "from-source"))`, so with the feature on there is nothing
// to degrade to and a silent return would just fail later, more confusingly.
// Enabling the feature is therefore a deliberate, platform-specific act — see
// the note in `crates/sync-p2p/Cargo.toml` about never forcing it workspace-wide.
//
// SYNC-9 must not undo this: `from-source` stays default-OFF and opt-in.
// Widening `supported()` below is the only sanctioned way to add a target —
// never make the feature default-on (see docs/internal/sync-p2p.md §15.2b).

use std::env;
use std::path::PathBuf;

fn supported() -> bool {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    matches!(
        (os.as_str(), arch.as_str()),
        ("linux", "x86_64")
            | ("linux", "aarch64")
            | ("macos", "aarch64")
            | ("macos", "x86_64")
            | ("windows", "x86_64")
    )
}

fn main() {
    // Feature OFF → nothing to do; the prebuilt path is untouched.
    if env::var_os("CARGO_FEATURE_FROM_SOURCE").is_none() {
        return;
    }

    if !supported() {
        panic!(
            "cloudsync `from-source` is only implemented for linux (x86_64/aarch64), \
             macOS (aarch64/x86_64), and Windows (x86_64); got {}-{}. Android is not \
             supported (needs vendor/src/network/cacert.h, tracked separately). \
             Build without `--features from-source` to use the prebuilt extension.",
            env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        );
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = manifest_dir.join("vendor").join("src");

    // SYNC-9 §21.14: `vendor/src/sqlite/sqlite3ext.h` does `#include
    // "sqlite3.h"`, and `sqlite3.h` itself is NOT vendored anywhere under
    // `vendor/` (only `sqlite3ext.h` is) — see docs/internal/sync-p2p.md
    // §21.13. Without an explicit include path this resolved through the
    // compiler's *implicit* system include path — `/usr/include` on linux,
    // the macOS SDK on Mac — which happened to exist on both, so it never
    // surfaced there. MSVC has no implicit system include path and Windows
    // does not ship SQLite headers at all, which is why this only broke on
    // Windows CI (run 33419275613): "fatal error C1083: Cannot open include
    // file: 'sqlite3.h'". Relying on an ambient system header is also a
    // latent ABI risk even where it "works": it could resolve to a different
    // SQLite version than the one actually linked into the app.
    //
    // Fix: `libsqlite3-sys` is a dependency of this crate (gated by this same
    // `from-source` feature — see Cargo.toml) and declares `links =
    // "sqlite3"`, so cargo forwards its build script's `cargo:include=...`
    // output to *this* build script as `DEP_SQLITE3_INCLUDE`. It must be a
    // normal [dependencies] entry, not [build-dependencies]: verified
    // empirically that Cargo does not forward DEP_<links>_<key> vars across a
    // build-dependency edge, only a normal/target-dependency one (see
    // Cargo.toml). That path belongs to the exact SQLite libsqlite3-sys
    // bundles/links for the rest of the app, so pointing cloudsync's own
    // compile at it — instead of an ambient system header — makes the header
    // and the linked library the same SQLite on every platform, not just by
    // accident on two of three.
    //
    // Fail loudly instead of silently falling back to a system header if the
    // var is absent: that fallback is exactly the bug this fixes.
    let sqlite_include = env::var("DEP_SQLITE3_INCLUDE").unwrap_or_else(|_| {
        panic!(
            "cloudsync `from-source` build failed: DEP_SQLITE3_INCLUDE is not set. This \
             variable is expected to come from libsqlite3-sys's build script (it declares \
             `links = \"sqlite3\"`, and libsqlite3-sys is a dependency of this crate gated \
             by the `from-source` feature — see Cargo.toml). Its absence means either \
             libsqlite3-sys's build script did not run before this one, or the dependency \
             is missing/misconfigured. Refusing to silently fall back to an ambient system \
             sqlite3.h: that header may not match the SQLite version actually linked into \
             the app (the ABI-mismatch risk this check exists to prevent). See \
             docs/internal/sync-p2p.md §21.14."
        )
    });
    println!(
        "cargo:warning=cloudsync from-source: using sqlite3.h from DEP_SQLITE3_INCLUDE={sqlite_include}"
    );

    let sources = [
        "block.c",
        "cloudsync.c",
        "dbutils.c",
        "lz4.c",
        "pk.c",
        "utils.c",
        "network/network.c",
        "sqlite/cloudsync_changes_sqlite.c",
        "sqlite/cloudsync_sqlite.c",
        "sqlite/database_sqlite.c",
        "sqlite/sql_sqlite.c",
        "modules/fractional-indexing/fractional_indexing.c",
    ];

    let include_dirs = [
        vendor.clone(),
        vendor.join("network"),
        vendor.join("sqlite"),
        vendor.join("modules/fractional-indexing"),
        PathBuf::from(sqlite_include),
    ];

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // S1: compile the real P2P network layer instead of the S0b stub. The stub
    // (network_stub.c) is retained on disk for reference but no longer built.
    let stub = manifest_dir.join("build").join("network_p2p.c");

    // -fPIC is a POSIX/ELF-and-Mach-O concept: meaningless on Windows (code is
    // always position-independent there) and MSVC's cl.exe rejects the flag
    // outright, so it is only added for non-Windows targets.
    let is_windows = target_os == "windows";

    // Compile each vendored source to an object file. One cc::Build per file
    // so we can collect the object paths from compile_intermediates().
    let mut objects = Vec::new();
    for src in sources {
        let path = vendor.join(src);
        let mut one = cc::Build::new();
        if !is_windows {
            one.flag("-fPIC");
        }
        one.define("CLOUDSYNC_OMIT_CURL", None)
            .opt_level(2)
            .warnings(false);
        for dir in &include_dirs {
            one.include(dir);
        }
        one.file(&path);
        objects.extend(one.compile_intermediates());
    }

    // Compile the custom-network stub (the S1 replace-point).
    let mut stub_build = cc::Build::new();
    if !is_windows {
        stub_build.flag("-fPIC");
    }
    stub_build
        .define("CLOUDSYNC_OMIT_CURL", None)
        .opt_level(2)
        .warnings(false);
    for dir in &include_dirs {
        stub_build.include(dir);
    }
    stub_build.file(&stub);
    objects.extend(stub_build.compile_intermediates());

    // Link the objects into a loadable shared object. Done manually (rather
    // than cc::Build::compile) so we control the output name and avoid
    // emitting `cargo:rustc-link-lib` directives that would link the .so into
    // the Rust binary instead of loading it at runtime.
    //
    // SYNC-9: the link step is genuinely platform-specific, unlike the rest of
    // this file:
    // - linux / macOS: GCC/clang accept `-shared`; macOS gets `-dynamiclib`
    //   explicitly (the flag `cc::Build`'s `.get_compiler()` driver uses to
    //   produce a real `.dylib`, matching the `.dylib` extension the prebuilt
    //   artifacts under `vendor/cloudsync/macos/` already use — see
    //   docs/internal/sync-p2p.md §21 for why this was verified rather than
    //   assumed).
    // - windows/MSVC (`cc::Build`'s compiler `is_like_msvc()`): `cl.exe` does
    //   not understand `-shared`/`-o`; `/LD` tells cl to produce a DLL and
    //   invoke `link.exe` itself, `/Fe:` sets the output path, and `ws2_32.lib`
    //   supplies the Winsock2 symbols `network_p2p.c` now calls.
    // - windows/GNU (mingw-w64, not used by this repo's CI but kept working
    //   for anyone building outside it): behaves like linux, plus `-lws2_32`.
    let compiler = cc::Build::new().get_compiler();
    let so_file_name = match target_os.as_str() {
        "macos" => "cloudsync.dylib",
        "windows" => "cloudsync.dll",
        _ => "cloudsync.so",
    };
    let so_path = out_dir.join(so_file_name);

    let mut cmd = compiler.to_command();
    if is_windows && compiler.is_like_msvc() {
        cmd.arg("/LD")
            .arg(format!("/Fe:{}", so_path.display()))
            .args(&objects)
            .arg("ws2_32.lib");
    } else {
        cmd.arg(if target_os == "macos" {
            "-dynamiclib"
        } else {
            "-shared"
        })
        .arg("-o")
        .arg(&so_path)
        .args(&objects);
        if target_os == "macos" {
            // vendor/src/utils.c calls SecRandomCopyBytes/kSecRandomDefault
            // (<Security/Security.h>) for UUID generation on Apple targets.
            // Observed as a real link failure on macos-15/arm64 CI (SYNC-9
            // first push, run 33410001375): "Undefined symbols ...
            // _SecRandomCopyBytes / _kSecRandomDefault ... ld: symbol(s) not
            // found for architecture arm64" — `-dynamiclib` alone does not
            // pull in the framework. `-framework CoreFoundation` was
            // considered and NOT added: nothing in vendor/src or
            // build/network_p2p.c calls any CF*/CoreFoundation symbol
            // (checked by grep), and Security.framework resolves its own
            // internal dependency on CoreFoundation without our object files
            // needing to reference it directly.
            cmd.arg("-framework").arg("Security");
        }
        if is_windows {
            cmd.arg("-lws2_32");
        } else {
            cmd.arg("-lm");
        }
    }

    let status = cmd.status().expect("failed to run C linker");
    assert!(
        status.success(),
        "failed to link cloudsync shared object from source"
    );

    println!("cargo:rerun-if-changed={}", vendor.display());
    println!("cargo:rerun-if-changed={}", stub.display());
    println!(
        "cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO={}",
        so_path.display()
    );
}
