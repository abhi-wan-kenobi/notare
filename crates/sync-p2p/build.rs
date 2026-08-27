// Builds the vendored sqlite-sync (CloudSync) extension with the S1 P2P
// network layer into a loadable shared object, so the `sync_two_nodes` example
// (and tests) can load it without depending on cargo injecting cloudsync's
// `cargo:rustc-env` into a *different* crate's binary.
//
// This duplicates crates/cloudsync/build.rs's recipe for the spike. The
// authoritative build remains cloudsync's; this one exists so the example
// binary's runtime `std::env::var("CLOUDSYNC_FROM_SOURCE_SO")` (inside
// cloudsync::apply) resolves. linux/x86_64 only (S0b/S1 scope).

use std::env;
use std::path::PathBuf;

fn supported() -> bool {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    os == "linux" && arch == "x86_64"
}

fn main() {
    // Only build when an example/test actually needs the extension. Skip for a
    // plain `cargo check` of the lib (no dev-deps compiled). We always emit the
    // env for the examples/tests when supported.
    if !supported() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cloudsync_dir = manifest_dir.join("..").join("cloudsync");
    let vendor = cloudsync_dir.join("vendor").join("src");

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
    ];

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let stub = cloudsync_dir.join("build").join("network_p2p.c");

    let mut objects = Vec::new();
    for src in sources {
        let path = vendor.join(src);
        let mut one = cc::Build::new();
        one.flag("-fPIC")
            .define("CLOUDSYNC_OMIT_CURL", None)
            .opt_level(2)
            .warnings(false);
        for dir in &include_dirs {
            one.include(dir);
        }
        one.file(&path);
        objects.extend(one.compile_intermediates());
    }

    let mut stub_build = cc::Build::new();
    stub_build
        .flag("-fPIC")
        .define("CLOUDSYNC_OMIT_CURL", None)
        .opt_level(2)
        .warnings(false);
    for dir in &include_dirs {
        stub_build.include(dir);
    }
    stub_build.file(&stub);
    objects.extend(stub_build.compile_intermediates());

    let compiler = cc::Build::new().get_compiler();
    let so_path = out_dir.join("cloudsync.so");
    let mut cmd = compiler.to_command();
    cmd.arg("-shared")
        .arg("-o")
        .arg(&so_path)
        .args(&objects)
        .arg("-lm");
    let status = cmd.status().expect("failed to run C linker");
    assert!(status.success(), "failed to link cloudsync.so for sync-p2p");

    println!("cargo:rerun-if-changed={}", vendor.display());
    println!("cargo:rerun-if-changed={}", stub.display());
    println!(
        "cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO={}",
        so_path.display()
    );
}
