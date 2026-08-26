// Builds the vendored sqlite-sync (CloudSync) extension from source into a
// loadable shared object, gated behind the `from-source` cargo feature.
//
// When the feature is OFF (the default), this script is a no-op and the crate
// keeps using the prebuilt `include_bytes!` artifacts in `vendor/cloudsync/`.
//
// Scope: linux/x86_64 only (S0b). Other targets fall through to the prebuilt
// path; extend the `supported()` check for SYNC-9.

use std::env;
use std::path::PathBuf;

fn supported() -> bool {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    os == "linux" && arch == "x86_64"
}

fn main() {
    // Feature OFF → nothing to do; the prebuilt path is untouched.
    if env::var_os("CARGO_FEATURE_FROM_SOURCE").is_none() {
        return;
    }

    if !supported() {
        panic!(
            "cloudsync `from-source` is only implemented for linux/x86_64 (S0b); \
             got {}-{}. Build without `--features from-source` to use the prebuilt extension.",
            env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default(),
        );
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = manifest_dir.join("vendor").join("src");

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
    let stub = manifest_dir.join("build").join("network_stub.c");

    // Compile each vendored source to an object file. One cc::Build per file
    // so we can collect the object paths from compile_intermediates().
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

    // Compile the custom-network stub (the S1 replace-point).
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

    // Link the objects into a loadable shared object. Done manually (rather
    // than cc::Build::compile) so we control the output name and avoid
    // emitting `cargo:rustc-link-lib` directives that would link the .so into
    // the Rust binary instead of loading it at runtime.
    let compiler = cc::Build::new().get_compiler();
    let so_path = out_dir.join("cloudsync.so");

    let mut cmd = compiler.to_command();
    cmd.arg("-shared")
        .arg("-o")
        .arg(&so_path)
        .args(&objects)
        .arg("-lm");

    let status = cmd.status().expect("failed to run C linker");
    assert!(status.success(), "failed to link cloudsync.so from source");

    println!("cargo:rerun-if-changed={}", vendor.display());
    println!("cargo:rerun-if-changed={}", stub.display());
    println!(
        "cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO={}",
        so_path.display()
    );
}
