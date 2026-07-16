//! Text injection into the currently focused application (Windows/Linux).
//!
//! Primary path: `enigo` synthesizes the text directly (Windows: `SendInput`
//! with `KEYEVENTF_UNICODE`, layout-independent; Linux X11: XTest via the
//! pure-Rust `x11rb` backend). Fallback path: put the text on the clipboard,
//! synthesize Ctrl+V, then restore the previous clipboard text.
//!
//! Wayland is a documented follow-up: enigo's default build has no Wayland
//! backend, so both paths error there and the caller surfaces it.

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

/// Blocking: call from a blocking-safe context (`spawn_blocking` in the
/// session loop; commands go through the same path).
pub fn type_text(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }

    match type_direct(text) {
        Ok(()) => Ok(()),
        Err(direct_error) => {
            tracing::warn!(
                error = %direct_error,
                "direct text injection failed; falling back to clipboard paste"
            );
            paste_via_clipboard(text).map_err(|paste_error| {
                Error::Inject(format!(
                    "direct injection failed ({direct_error}); clipboard fallback failed ({paste_error})"
                ))
            })
        }
    }
}

fn type_direct(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())
}

/// Save the current clipboard text, replace it with `text`, synthesize
/// Ctrl+V, then restore. Non-text clipboard contents (images, files) are not
/// restored - acceptable for a fallback path.
fn paste_via_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let saved = clipboard.get_text().ok();

    clipboard
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;
    std::thread::sleep(PRE_PASTE_DELAY);

    let paste_result = send_paste_chord();

    std::thread::sleep(POST_PASTE_DELAY);
    if let Some(previous) = saved
        && let Err(error) = clipboard.set_text(previous)
    {
        tracing::warn!(%error, "failed to restore previous clipboard contents");
    }

    paste_result
}

fn send_paste_chord() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Windows: use the virtual-key code for `V` (0x56) so the chord is
    // keyboard-layout independent. Elsewhere: a unicode `v` keysym.
    #[cfg(windows)]
    let v_key = Key::Other(0x56);
    #[cfg(not(windows))]
    let v_key = Key::Unicode('v');

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    let click = enigo
        .key(v_key, Direction::Click)
        .map_err(|e| e.to_string());
    // Hold the chord briefly so slow apps register it as Ctrl+V.
    std::thread::sleep(CHORD_HOLD);
    // Always release the modifier, even if the paste click failed.
    let release = enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string());

    click.and(release)
}
