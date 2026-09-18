//! Verify the vendored prebuilt cr-sqlite (vlcn-io v0.16.3) loadable
//! extension binaries against `vendor/SHA256SUMS` at build time.
//!
//! The binaries are embedded with `include_bytes!` (src/bundle.rs) and
//! extracted to the user's cache dir at runtime, so a corrupted or tampered
//! vendor file would otherwise fail only at first app run — on the user's
//! disk. Verifying at build time pins the supply chain to exactly the
//! checksums recorded when the binaries were vendored (which are themselves
//! the release-artifact checksums from the 2026-09-07 plan).
//!
//! `sha2` (a build-dependency) is the same hasher the release process uses
//! for `SHA256SUMS`.

use std::io::BufRead;
use std::path::PathBuf;

fn main() {
    // Build scripts run with CWD = the workspace root, not the package
    // root — anchor every path at CARGO_MANIFEST_DIR.
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );

    println!("cargo:rerun-if-changed=vendor/SHA256SUMS");
    for entry in [
        "linux-x86_64",
        "darwin-aarch64",
        "darwin-x86_64",
        "win-x86_64",
    ] {
        println!("cargo:rerun-if-changed=vendor/{entry}/");
    }

    let sums_path = manifest_dir.join("vendor/SHA256SUMS");
    let file = match std::fs::File::open(&sums_path) {
        Ok(file) => file,
        Err(error) => panic!(
            "crsqlite build.rs: cannot open {}: {error}",
            sums_path.display()
        ),
    };

    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("read SHA256SUMS line");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (expected, rel_path) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("crsqlite build.rs: malformed SHA256SUMS line: {line:?}"));

        let path = manifest_dir.join(rel_path);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => panic!("crsqlite build.rs: cannot read {}: {error}", path.display()),
        };

        use sha2::{Digest, Sha256};
        let actual = hex(&Sha256::digest(&bytes));

        assert_eq!(
            expected,
            actual,
            "crsqlite build.rs: vendored binary {} does not match its recorded SHA256SUMS \
             entry — was it corrupted, or edited without updating SHA256SUMS?",
            path.display()
        );
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
