//! SYNC-9 regression test for the §12/§13.9 carried finding: a bracketed
//! IPv6 `NOTARE_SYNC_AGENT_ADDR`, e.g. "[::1]:1234", must resolve to host
//! "::1" (brackets stripped) before it reaches `getaddrinfo()`, which rejects
//! the bracket syntax outright.
//!
//! `resolve_agent_addr_from` (the pure host:port parser, extracted into
//! `build/agent_addr.h` specifically so it is testable without pulling in the
//! rest of `build/network_p2p.c`'s dependencies — the cloudsync SQLite
//! allocator, sockets, the vendored build) is exercised here by compiling a
//! tiny, self-contained C harness against that header and running it as a
//! subprocess. This does not require the `from-source` C build machinery
//! (no vendored sources, no linking), only a C compiler in `PATH` (`$CC` or
//! `cc`), which `from-source` already requires.

#![cfg(feature = "from-source")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn agent_addr_header_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build")
}

/// Compile `harness_c` (a full C `main()`, expected to `#include
/// "agent_addr.h"`) and run it, returning the captured output. Panics loudly
/// on a compile failure so a broken harness never reads as "the C code is
/// fine, the harness just didn't build."
fn compile_and_run(name: &str, harness_c: &str) -> Output {
    let header_dir = agent_addr_header_dir();

    let scratch = std::env::temp_dir().join(format!(
        "cloudsync-agent-addr-test-{}-{}",
        std::process::id(),
        name
    ));
    std::fs::create_dir_all(&scratch).expect("create scratch dir for C test harness");

    let c_path = scratch.join("harness.c");
    std::fs::write(&c_path, harness_c).expect("write C test harness");

    let exe_path: PathBuf = scratch.join(if cfg!(windows) {
        "harness.exe"
    } else {
        "harness"
    });

    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let compile_status = Command::new(&cc)
        .arg("-I")
        .arg(&header_dir)
        .arg("-o")
        .arg(&exe_path)
        .arg(&c_path)
        .status()
        .unwrap_or_else(|e| panic!("failed to invoke `{cc}` to compile the test harness: {e}"));
    assert!(
        compile_status.success(),
        "C test harness failed to compile:\n{}",
        harness_c
    );

    run(&exe_path)
}

fn run(exe_path: &Path) -> Output {
    Command::new(exe_path)
        .output()
        .unwrap_or_else(|e| panic!("failed to run compiled test harness {exe_path:?}: {e}"))
}

const PRELUDE: &str = r#"
#include "agent_addr.h"
#include <stdio.h>
#include <string.h>
"#;

fn assert_harness_ok(name: &str, body: &str) {
    let harness = format!("{PRELUDE}\nint main(void) {{\n{body}\n    return 0;\n}}\n");
    let out = compile_and_run(name, &harness);
    assert!(
        out.status.success(),
        "{name}: harness assertion failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn resolve_agent_addr_strips_ipv6_brackets() {
    assert_harness_ok(
        "ipv6_brackets",
        r#"
    char host[256];
    int port = 0;
    bool ok = resolve_agent_addr_from("[::1]:4433", host, sizeof(host), &port);
    if (!ok) { fprintf(stderr, "expected ok=true\n"); return 1; }
    if (strcmp(host, "::1") != 0) { fprintf(stderr, "host=%s (want ::1)\n", host); return 1; }
    if (port != 4433) { fprintf(stderr, "port=%d (want 4433)\n", port); return 1; }
"#,
    );
}

#[test]
fn resolve_agent_addr_ipv4_is_unaffected() {
    assert_harness_ok(
        "ipv4_unaffected",
        r#"
    char host[256];
    int port = 0;
    bool ok = resolve_agent_addr_from("127.0.0.1:9999", host, sizeof(host), &port);
    if (!ok) { fprintf(stderr, "expected ok=true\n"); return 1; }
    if (strcmp(host, "127.0.0.1") != 0) { fprintf(stderr, "host=%s (want 127.0.0.1)\n", host); return 1; }
    if (port != 9999) { fprintf(stderr, "port=%d (want 9999)\n", port); return 1; }
"#,
    );
}

#[test]
fn resolve_agent_addr_missing_fails_closed() {
    assert_harness_ok(
        "missing_fails_closed",
        r#"
    char host[256];
    int port = 0;
    if (resolve_agent_addr_from(NULL, host, sizeof(host), &port)) { fprintf(stderr, "expected ok=false for NULL\n"); return 1; }
    if (resolve_agent_addr_from("", host, sizeof(host), &port)) { fprintf(stderr, "expected ok=false for empty\n"); return 1; }
    if (resolve_agent_addr_from("no-port-here", host, sizeof(host), &port)) { fprintf(stderr, "expected ok=false with no colon\n"); return 1; }
"#,
    );
}

#[test]
fn resolve_agent_addr_oversized_host_fails_closed() {
    // host_cap bounds check must still reject a host that doesn't fit,
    // unaffected by the bracket-stripping fix (§12 carried: fails closed, no
    // memory-safety consequence).
    assert_harness_ok(
        "oversized_host_bounds_check",
        r#"
    char host[4];
    int port = 0;
    if (resolve_agent_addr_from("[::1]:4433", host, sizeof(host), &port)) {
        fprintf(stderr, "expected ok=false: host_cap=4 is too small for \"[::1]\"\n");
        return 1;
    }
"#,
    );
}
