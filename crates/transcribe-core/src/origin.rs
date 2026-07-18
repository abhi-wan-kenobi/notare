//! Shared cross-origin allowlist for the Notare STT companion server
//! (`apps/stt-server`) — used by **both** [`crate::service::streaming`]'s
//! `/v1/listen` WebSocket-Origin check (SEC-02, Cross-Site WebSocket
//! Hijacking) and `apps/stt-server/src/router.rs`'s `CorsLayer` (SEC-01,
//! `cors::Any`). One list, two enforcement points, so they can never drift
//! apart — see `SECURITY-REVIEW.md`.
//!
//! ## Allowed origins
//!
//! - `tauri://localhost` — the Tauri v2 webview origin on macOS and Linux.
//! - `http://tauri.localhost` — the Tauri v2 webview origin on Windows (and
//!   Android, not a Notare target).
//!
//!   Verified against the vendored `tauri` 2.10.3 source
//!   (`tauri::manager::AppManager::tauri_protocol_url`,
//!   `tauri-2.10.3/src/manager/mod.rs:329-338`): every platform except
//!   Windows/Android gets the `tauri://` custom scheme directly; those two
//!   get the `{http,https}://tauri.localhost` `wry` workaround instead
//!   (WebView2/WebView-on-Android can't navigate a bare custom scheme).
//!   Notare's `tauri.conf*.json` files never set `useHttpsScheme` (it
//!   defaults to `false`), so the `https://tauri.localhost` variant never
//!   applies here.
//! - `http://localhost[:PORT]` / `http://127.0.0.1[:PORT]`, any port
//!   (including none) — dev-mode origins. The desktop's `devUrl` is
//!   `http://localhost:1422` today (`apps/desktop/src-tauri/tauri.conf.json`),
//!   but other dev tooling may pick a different port, so this matches on
//!   host only, not a fixed port.
//!
//! ## In practice, neither real cross-origin caller needs this at all
//!
//! The desktop's "Test connection" button
//! (`apps/desktop/src/settings/ai/stt/connection-test.ts`) calls
//! `@tauri-apps/plugin-http`'s `fetch`, which runs the HTTP request from the
//! **Rust backend**, not the webview's networking stack — it is never
//! subject to browser CORS or WS-Origin checks at all (confirmed against
//! `@tauri-apps/plugin-http`'s own README: "Access the HTTP client written
//! in Rust"). The real transcription client
//! (`crates/owhisper-client`, used by both batch and live) is plain
//! `reqwest`/a native WS client — also not browser-bound. The embedded admin
//! page (`apps/stt-server/src/assets/index.html`) only ever calls
//! same-origin, which never triggers CORS or sends an `Origin` header
//! WebSocket-side in the first place.
//!
//! So this allowlist exists purely as **defense-in-depth**: it protects
//! against a future plain `window.fetch`/`new WebSocket(...)` from the
//! webview, and it is what keeps `cors::Any` from ever coming back. It must
//! never be widened to accept arbitrary origins.

use axum::http::HeaderValue;

const ALLOWED_STATIC_ORIGINS: [&str; 2] = ["tauri://localhost", "http://tauri.localhost"];

/// `true` if `origin` (an HTTP `Origin` header value) is on the shared
/// allowlist. Callers decide separately what to do with a *missing* Origin
/// header — this function only judges a header that is actually present.
pub fn is_allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    ALLOWED_STATIC_ORIGINS.contains(&origin) || is_localhost_dev_origin(origin)
}

/// `http://localhost` or `http://127.0.0.1`, optionally with `:<port>`.
/// Deliberately `http`-only — nothing on this allowlist needs `https`.
fn is_localhost_dev_origin(origin: &str) -> bool {
    for host in ["http://localhost", "http://127.0.0.1"] {
        let Some(rest) = origin.strip_prefix(host) else {
            continue;
        };
        if rest.is_empty() {
            return true;
        }
        if let Some(port) = rest.strip_prefix(':') {
            if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).unwrap()
    }

    #[test]
    fn allows_tauri_macos_linux_origin() {
        assert!(is_allowed_origin(&header("tauri://localhost")));
    }

    #[test]
    fn allows_tauri_windows_origin() {
        assert!(is_allowed_origin(&header("http://tauri.localhost")));
    }

    #[test]
    fn rejects_https_tauri_localhost() {
        // Notare never sets `useHttpsScheme`, so this variant is never
        // legitimate and should not be pre-emptively allowed.
        assert!(!is_allowed_origin(&header("https://tauri.localhost")));
    }

    #[test]
    fn allows_bare_localhost_dev_origin() {
        assert!(is_allowed_origin(&header("http://localhost")));
        assert!(is_allowed_origin(&header("http://127.0.0.1")));
    }

    #[test]
    fn allows_localhost_dev_origin_at_any_port() {
        assert!(is_allowed_origin(&header("http://localhost:1422")));
        assert!(is_allowed_origin(&header("http://localhost:5173")));
        assert!(is_allowed_origin(&header("http://127.0.0.1:9999")));
    }

    #[test]
    fn rejects_https_localhost() {
        assert!(!is_allowed_origin(&header("https://localhost:1422")));
    }

    #[test]
    fn rejects_a_malicious_origin() {
        assert!(!is_allowed_origin(&header("http://malicious-site.com")));
        assert!(!is_allowed_origin(&header("https://evil.example")));
    }

    #[test]
    fn rejects_a_localhost_lookalike_origin() {
        // Must not substring-match — `localhost.evil.example` is not
        // `localhost`.
        assert!(!is_allowed_origin(&header(
            "http://localhost.evil.example"
        )));
        assert!(!is_allowed_origin(&header("http://notlocalhost:1422")));
    }
}
