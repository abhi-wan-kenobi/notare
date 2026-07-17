//! Text injection into the currently focused application (Windows/Linux/
//! macOS).
//!
//! How text reaches the focused app depends on the [`InjectionStrategy`],
//! resolved once per process:
//!
//! - **Enigo** (Windows, macOS, Linux/X11): `enigo` synthesizes the text
//!   directly (Windows: `SendInput` with `KEYEVENTF_UNICODE`, layout-
//!   independent; macOS: `CGEvent` unicode key events - requires the
//!   Accessibility permission, see `plugins/permissions`; Linux X11: XTest
//!   via the pure-Rust `x11rb` backend). Fallback path: put the text on the
//!   clipboard, synthesize the platform paste chord (Cmd+V on macOS, Ctrl+V
//!   elsewhere), then restore the previous clipboard text.
//! - **Wtype** (Linux/Wayland with the `wtype` binary on `PATH`): Wayland
//!   compositors don't implement XTEST, so enigo either errors or silently
//!   no-ops there. `wtype` speaks the `zwp_virtual_keyboard_v1` protocol and
//!   can both type text and send the Ctrl+V paste chord.
//! - **ClipboardOnly** (Linux/Wayland without `wtype`): synthetic input is
//!   impossible; every "injection" degrades to copying the text to the
//!   clipboard so the user can paste it themselves. The dictation session
//!   surfaces this via `DictationFinishedEvent.injection_fallback` so the UI
//!   can tell the user (see `session.rs`).

use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::Error;

/// How long the clipboard fallback waits between setting the clipboard and
/// sending Ctrl+V (lets the clipboard write settle), and between the paste and
/// restoring the previous contents (lets the target app read the clipboard).
const PRE_PASTE_DELAY: Duration = Duration::from_millis(80);
const POST_PASTE_DELAY: Duration = Duration::from_millis(200);
/// How long the Ctrl+V chord is held before releasing the modifier (matches
/// the reference dictation app's proven timing).
const CHORD_HOLD: Duration = Duration::from_millis(100);

/// How dictated text reaches the focused app on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionStrategy {
    /// Synthetic input via enigo (Windows `SendInput` / macOS `CGEvent` /
    /// Linux X11 XTEST).
    Enigo,
    /// Wayland session with the `wtype` virtual-keyboard tool on `PATH`.
    Wtype,
    /// Wayland session without `wtype`: no synthetic input is possible, text
    /// can only be copied to the clipboard.
    ClipboardOnly,
}

impl InjectionStrategy {
    pub fn is_clipboard_only(self) -> bool {
        self == Self::ClipboardOnly
    }
}

/// The process-wide injection strategy, resolved on first use and cached
/// (session type and `PATH` don't change mid-run).
pub fn strategy() -> InjectionStrategy {
    #[cfg(not(target_os = "linux"))]
    {
        InjectionStrategy::Enigo
    }
    #[cfg(target_os = "linux")]
    {
        static STRATEGY: std::sync::OnceLock<InjectionStrategy> = std::sync::OnceLock::new();
        *STRATEGY.get_or_init(|| {
            let resolved = resolve_strategy(|key| std::env::var(key).ok(), wtype_on_path());
            tracing::info!(strategy = ?resolved, "resolved text-injection strategy");
            resolved
        })
    }
}

/// Pure strategy resolution, separated from process env / `PATH` lookup so it
/// is unit-testable. `env` returns the value of an environment variable,
/// `wtype_available` whether a `wtype` binary was found on `PATH`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn resolve_strategy(
    env: impl Fn(&str) -> Option<String>,
    wtype_available: bool,
) -> InjectionStrategy {
    if !is_wayland_session(&env) {
        return InjectionStrategy::Enigo;
    }
    if wtype_available {
        InjectionStrategy::Wtype
    } else {
        InjectionStrategy::ClipboardOnly
    }
}

/// Wayland detection: an explicit `XDG_SESSION_TYPE` wins; otherwise a set
/// `WAYLAND_DISPLAY` with no X11 `DISPLAY` to fall back on counts as Wayland.
/// (With both set - XWayland - enigo's XTEST path still works for the X11
/// server, so an explicit `XDG_SESSION_TYPE=wayland` is required to switch.)
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn is_wayland_session(env: &impl Fn(&str) -> Option<String>) -> bool {
    match env("XDG_SESSION_TYPE").as_deref() {
        Some(session_type) if session_type.eq_ignore_ascii_case("wayland") => true,
        Some(session_type) if session_type.eq_ignore_ascii_case("x11") => false,
        _ => {
            let wayland = env("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
            let x11 = env("DISPLAY").is_some_and(|v| !v.is_empty());
            wayland && !x11
        }
    }
}

/// Whether a `wtype` binary exists in any `PATH` directory.
#[cfg(target_os = "linux")]
fn wtype_on_path() -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join("wtype").is_file())
}

/// Blocking: call from a blocking-safe context (`spawn_blocking` in the
/// session loop; commands go through the same path).
///
/// Under the `ClipboardOnly` strategy nothing can be typed - the text is
/// copied to the clipboard instead and `Ok` is returned; callers that need to
/// distinguish should check [`strategy()`] up front (the dictation session
/// does, and skips per-segment injection entirely).
pub fn type_text(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }

    match strategy() {
        InjectionStrategy::Enigo => match type_direct(text) {
            Ok(()) => Ok(()),
            Err(direct_error) => {
                tracing::warn!(
                    error = %direct_error,
                    "direct text injection failed; falling back to clipboard paste"
                );
                paste_via_clipboard(text, true).map_err(|paste_error| {
                    Error::Inject(format!(
                        "direct injection failed ({direct_error}); clipboard fallback failed ({paste_error})"
                    ))
                })
            }
        },
        InjectionStrategy::Wtype => wtype_text(text).map_err(|wtype_error| {
            // A failing wtype means the chord path would fail too; preserve
            // the text on the clipboard so nothing is lost, but report the
            // injection failure to the caller.
            tracing::warn!(error = %wtype_error, "wtype injection failed; copying to clipboard instead");
            if let Err(copy_error) = copy_text(text) {
                tracing::warn!(error = %copy_error, "clipboard copy after wtype failure also failed");
            }
            Error::Inject(format!("wtype injection failed ({wtype_error})"))
        }),
        InjectionStrategy::ClipboardOnly => {
            tracing::debug!("clipboard-only strategy: copying text instead of typing");
            copy_text(text)
        }
    }
}

/// Batch-paste delivery: put `text` on the clipboard and synthesize Ctrl+V.
/// Unlike the type-mode fallback, the previous clipboard contents are NOT
/// restored - the dictated text intentionally stays available for repeated
/// pastes. Blocking, like [`type_text`].
///
/// Under the `ClipboardOnly` strategy the paste chord cannot be synthesized;
/// this degrades to copy-only (the finished-event's `injection_fallback` flag
/// lets the UI tell the user to press Ctrl+V themselves).
pub fn paste_text(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }
    if strategy().is_clipboard_only() {
        tracing::info!("clipboard-only strategy: copying without synthesizing a paste");
        return copy_text(text);
    }
    paste_via_clipboard(text, false).map_err(Error::Inject)
}

/// Put `text` on the clipboard without pasting (used to preserve a batch
/// transcript when the session died before a clean stop, and as the terminal
/// fallback under Wayland).
pub fn copy_text(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }
    let mut clipboard = arboard::Clipboard::new().map_err(|e| Error::Inject(e.to_string()))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| Error::Inject(e.to_string()))
}

fn type_direct(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())
}

/// Type `text` with `wtype`, feeding it through stdin (`wtype -`) so text
/// starting with `-` or containing anything argv-hostile is passed verbatim.
fn wtype_text(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("wtype")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn wtype: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("failed to write text to wtype stdin: {e}"))?;
        // Drop closes the pipe so wtype sees EOF.
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for wtype: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "wtype exited with {}: {}",
            output.status,
            stderr.trim()
        ))
    }
}

/// Send the Ctrl+V chord with `wtype` (Wayland).
fn wtype_paste_chord() -> Result<(), String> {
    use std::process::Command;

    let output = Command::new("wtype")
        .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
        .output()
        .map_err(|e| format!("failed to run wtype paste chord: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "wtype paste chord exited with {}: {}",
            output.status,
            stderr.trim()
        ))
    }
}

/// Replace the clipboard text with `text` and synthesize Ctrl+V. With
/// `restore` the previous clipboard text is saved and put back afterwards
/// (type-mode fallback); non-text clipboard contents (images, files) are not
/// restored - acceptable for a fallback path. Without `restore` the pasted
/// text stays on the clipboard (batch-paste mode).
fn paste_via_clipboard(text: &str, restore: bool) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let saved = if restore { clipboard.get_text().ok() } else { None };

    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;
    std::thread::sleep(PRE_PASTE_DELAY);

    let paste_result = send_paste_chord();

    if restore {
        std::thread::sleep(POST_PASTE_DELAY);
        if let Some(previous) = saved
            && let Err(error) = clipboard.set_text(previous)
        {
            tracing::warn!(%error, "failed to restore previous clipboard contents");
        }
    }

    paste_result
}

fn send_paste_chord() -> Result<(), String> {
    if strategy() == InjectionStrategy::Wtype {
        return wtype_paste_chord();
    }

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Windows: use the virtual-key code for `V` (0x56) so the chord is
    // keyboard-layout independent. Elsewhere: a unicode `v` keysym.
    #[cfg(windows)]
    let v_key = Key::Other(0x56);
    #[cfg(not(windows))]
    let v_key = Key::Unicode('v');

    // The paste chord's modifier: macOS pastes with Cmd+V (enigo's `Meta`,
    // not `Control` - the literal Control key is a different, unmapped
    // shortcut in virtually every macOS app). Everywhere else it's Ctrl+V.
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    let click = enigo
        .key(v_key, Direction::Click)
        .map_err(|e| e.to_string());
    // Hold the chord briefly so slow apps register it as Ctrl+V/Cmd+V.
    std::thread::sleep(CHORD_HOLD);
    // Always release the modifier, even if the paste click failed.
    let release = enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string());

    click.and(release)
}

#[cfg(test)]
mod tests {
    use super::{InjectionStrategy, resolve_strategy};
    use std::collections::HashMap;

    fn env_fn(
        vars: &[(&'static str, &'static str)],
    ) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<&'static str, &'static str> = vars.iter().copied().collect();
        move |key: &str| map.get(key).map(|v| v.to_string())
    }

    #[test]
    fn x11_session_uses_enigo() {
        let env = env_fn(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0")]);
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Enigo);
        assert_eq!(resolve_strategy(&env, false), InjectionStrategy::Enigo);
    }

    #[test]
    fn no_session_hints_uses_enigo() {
        let env = env_fn(&[]);
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Enigo);
    }

    #[test]
    fn explicit_wayland_session_prefers_wtype() {
        let env = env_fn(&[("XDG_SESSION_TYPE", "wayland"), ("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Wtype);
    }

    #[test]
    fn explicit_wayland_session_without_wtype_is_clipboard_only() {
        let env = env_fn(&[("XDG_SESSION_TYPE", "wayland")]);
        assert_eq!(
            resolve_strategy(&env, false),
            InjectionStrategy::ClipboardOnly
        );
    }

    #[test]
    fn explicit_wayland_wins_over_xwayland_display() {
        // XDG_SESSION_TYPE=wayland with DISPLAY set (XWayland) is still a
        // Wayland session: XTEST only reaches X11 apps there.
        let env = env_fn(&[
            ("XDG_SESSION_TYPE", "wayland"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DISPLAY", ":0"),
        ]);
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Wtype);
    }

    #[test]
    fn wayland_display_without_x11_fallback_counts_as_wayland() {
        let env = env_fn(&[("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!(
            resolve_strategy(&env, false),
            InjectionStrategy::ClipboardOnly
        );
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Wtype);
    }

    #[test]
    fn wayland_display_with_x11_fallback_stays_on_enigo() {
        // No explicit session type but an X11 DISPLAY exists: enigo can
        // reach the X server, keep the direct path.
        let env = env_fn(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")]);
        assert_eq!(resolve_strategy(&env, true), InjectionStrategy::Enigo);
    }

    #[test]
    fn explicit_x11_session_type_overrides_wayland_display() {
        let env = env_fn(&[("XDG_SESSION_TYPE", "x11"), ("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!(resolve_strategy(&env, false), InjectionStrategy::Enigo);
    }

    #[test]
    fn empty_env_values_are_ignored() {
        let env = env_fn(&[("WAYLAND_DISPLAY", ""), ("DISPLAY", "")]);
        assert_eq!(resolve_strategy(&env, false), InjectionStrategy::Enigo);
    }
}
