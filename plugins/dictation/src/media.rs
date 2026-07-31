//! Media auto-pause for dictation.
//!
//! When the `dictation_pause_media` setting is on, the frontend asks us to
//! pause whatever is currently PLAYING the moment a dictation session starts,
//! and to resume it when the session ends. The hard rule is: only ever resume
//! what WE paused. If the user had already paused their music, or paused it
//! themselves mid-dictation, we must not un-pause it. That's why `pause_playing`
//! returns the set of targets it actually paused and `resume` only touches that
//! exact set (remembered in [`MediaPauseState`]).
//!
//! Everything here is strictly best-effort: any error is swallowed (logged at
//! debug) and degrades to "paused/resumed nothing". Media control must never
//! fail a dictation or delay the mic, so the callers run this off the async
//! executor (`spawn_blocking`) and fire-and-forget.
//!
//! Platform backends:
//! - **Linux**: MPRIS over the D-Bus session bus (`org.mpris.MediaPlayer2.*`) -
//!   real. Enumerates players, pauses the ones reporting `Playing`.
//! - **Windows**: GSMTC (`Windows.Media.Control`) - real. Pauses the sessions
//!   whose playback status is `Playing`, keyed by source app id.
//! - **macOS**: best-effort via `osascript` telling Music/Spotify to pause -
//!   only those two apps, and only if already running + playing.
//! - **Anything else**: a no-op stub that returns "paused nothing" and logs once.

use std::sync::Mutex;

/// Remembers the platform-specific identifiers of the media targets THIS app
/// paused (MPRIS bus names on Linux, source-app ids on Windows, app names on
/// macOS), so `resume` only ever un-pauses players the user had playing when
/// dictation began - never something they paused themselves.
#[derive(Default)]
pub struct MediaPauseState {
    paused: Mutex<Vec<String>>,
    /// Serializes whole pause/resume operations (not just the bookkeeping):
    /// a resume racing into the window between "pause enumerated the players"
    /// and "record stored them" would drain an empty list and strand the
    /// media paused forever.
    op_lock: tokio::sync::Mutex<()>,
}

impl MediaPauseState {
    pub(crate) async fn lock_op(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.op_lock.lock().await
    }

    /// Record targets we just paused. Deduped so a repeated pause (e.g. a
    /// double session-start) can't queue the same player twice.
    pub(crate) fn record(&self, paused: Vec<String>) {
        if paused.is_empty() {
            return;
        }
        let mut guard = self.paused.lock().unwrap_or_else(|e| e.into_inner());
        for target in paused {
            if !guard.contains(&target) {
                guard.push(target);
            }
        }
    }

    /// Drain everything we paused, handing it to `resume`. Draining (not
    /// copying) makes resume idempotent: a second call resumes nothing.
    pub(crate) fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.paused.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Pause every currently-playing media target, returning the platform-specific
/// identifiers of the ones actually paused (so the caller can remember them for
/// resume). Heavy/blocking platform work - run it on the blocking pool. Errors
/// degrade to an empty vec (paused nothing).
pub fn pause_playing() -> Vec<String> {
    imp::pause_playing()
}

/// Resume the given targets (identifiers previously returned by
/// [`pause_playing`]). Blocking; errors are swallowed. The caller must only
/// pass targets WE paused - never resume something the user paused themselves.
pub fn resume(targets: Vec<String>) {
    if targets.is_empty() {
        return;
    }
    imp::resume(targets);
}

#[cfg(target_os = "linux")]
mod imp {
    //! MPRIS over the D-Bus session bus. `zbus`'s blocking API is used inside
    //! the caller's `spawn_blocking`, so no async executor is needed.

    use zbus::blocking::{Connection, Proxy, fdo::DBusProxy};

    const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
    const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

    pub fn pause_playing() -> Vec<String> {
        match try_pause_playing() {
            Ok(paused) => paused,
            Err(error) => {
                tracing::debug!(%error, "dictation media auto-pause (MPRIS) failed; skipping");
                Vec::new()
            }
        }
    }

    pub fn resume(targets: Vec<String>) {
        if let Err(error) = try_resume(&targets) {
            tracing::debug!(%error, "dictation media resume (MPRIS) failed; skipping");
        }
    }

    fn try_pause_playing() -> zbus::Result<Vec<String>> {
        let conn = Connection::session()?;
        let dbus = DBusProxy::new(&conn)?;
        let mut paused = Vec::new();
        for name in dbus.list_names()? {
            let name = name.as_str();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            let Ok(proxy) = player_proxy(&conn, name) else {
                continue;
            };
            // Only pause players actually playing; a paused/stopped one must be
            // left untouched so we never resume something the user had paused.
            if proxy
                .get_property::<String>("PlaybackStatus")
                .ok()
                .as_deref()
                == Some("Playing")
                && proxy.call_method("Pause", &()).is_ok()
            {
                paused.push(name.to_string());
            }
        }
        Ok(paused)
    }

    fn try_resume(targets: &[String]) -> zbus::Result<()> {
        let conn = Connection::session()?;
        for name in targets {
            if let Ok(proxy) = player_proxy(&conn, name) {
                // The player may have quit meanwhile - ignore per-player errors.
                let _ = proxy.call_method("Play", &());
            }
        }
        Ok(())
    }

    fn player_proxy<'a>(conn: &Connection, name: &str) -> zbus::Result<Proxy<'a>> {
        Proxy::new(conn, name.to_string(), MPRIS_PATH, PLAYER_IFACE)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    //! GSMTC (`Windows.Media.Control`). Sessions are keyed by their source app
    //! user-model id so resume can re-find the exact players we paused.

    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    };

    /// WinRT calls need an initialized COM apartment, and the blocking-pool
    /// threads we run on have none. RAII: init MTA on entry, uninit on drop -
    /// but only when WE initialized (RPC_E_CHANGED_MODE = the thread already
    /// has an STA; WinRT still works and we must not CoUninitialize it).
    struct ComApartment {
        owns: bool,
    }

    impl ComApartment {
        fn enter() -> Self {
            use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
            // S_OK / S_FALSE = (re)initialized -> pair with CoUninitialize.
            // Failure (e.g. RPC_E_CHANGED_MODE) -> not ours to uninit.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            Self { owns: hr.is_ok() }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.owns {
                unsafe { windows::Win32::System::Com::CoUninitialize() };
            }
        }
    }

    pub fn pause_playing() -> Vec<String> {
        let _com = ComApartment::enter();
        match try_pause_playing() {
            Ok(paused) => paused,
            Err(error) => {
                tracing::debug!(%error, "dictation media auto-pause (GSMTC) failed; skipping");
                Vec::new()
            }
        }
    }

    pub fn resume(targets: Vec<String>) {
        let _com = ComApartment::enter();
        if let Err(error) = try_resume(&targets) {
            tracing::debug!(%error, "dictation media resume (GSMTC) failed; skipping");
        }
    }

    fn try_pause_playing() -> zbus::Result<Vec<String>> {
        let conn = Connection::session()?;
        let dbus = DBusProxy::new(&conn)?;
        let mut paused = Vec::new();
        for name in dbus.list_names()? {
            let name = name.as_str();
            if !name.starts_with(MPRIS_PREFIX) {
                continue;
            }
            let Ok(proxy) = player_proxy(&conn, name) else {
                continue;
            };
            // Only pause players actually playing; a paused/stopped one must be
            // left untouched so we never resume something the user had paused.
            if proxy
                .get_property::<String>("PlaybackStatus")
                .ok()
                .as_deref()
                == Some("Playing")
                && proxy.call_method("Pause", &()).is_ok()
            {
                paused.push(name.to_string());
            }
        }
        Ok(paused)
    }

    fn try_resume(targets: &[String]) -> zbus::Result<()> {
        let conn = Connection::session()?;
        for name in targets {
            if let Ok(proxy) = player_proxy(&conn, name) {
                // The player may have quit meanwhile - ignore per-player errors.
                let _ = proxy.call_method("Play", &());
            }
        }
        Ok(())
    }

    fn player_proxy<'a>(conn: &Connection, name: &str) -> zbus::Result<Proxy<'a>> {
        Proxy::new(conn, name.to_string(), MPRIS_PATH, PLAYER_IFACE)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    //! GSMTC (`Windows.Media.Control`). Sessions are keyed by their source app
    //! user-model id so resume can re-find the exact players we paused.

    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    };

    /// WinRT calls need an initialized COM apartment, and the blocking-pool
    /// threads we run on have none. RAII: init MTA on entry, uninit on drop -
    /// but only when WE initialized (RPC_E_CHANGED_MODE = the thread already
    /// has an STA; WinRT still works and we must not CoUninitialize it).
    struct ComApartment {
        owns: bool,
    }

    impl ComApartment {
        fn enter() -> Self {
            use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
            // S_OK / S_FALSE = (re)initialized -> pair with CoUninitialize.
            // Failure (e.g. RPC_E_CHANGED_MODE) -> not ours to uninit.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            Self { owns: hr.is_ok() }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.owns {
                unsafe { windows::Win32::System::Com::CoUninitialize() };
            }
        }
    }

    pub fn pause_playing() -> Vec<String> {
        let _com = ComApartment::enter();
        match try_pause_playing() {
            Ok(paused) => paused,
            Err(error) => {
                tracing::debug!(%error, "dictation media auto-pause (GSMTC) failed; skipping");
                Vec::new()
            }
        }
    }

    pub fn resume(targets: Vec<String>) {
        let _com = ComApartment::enter();
        if let Err(error) = try_resume(&targets) {
            tracing::debug!(%error, "dictation media resume (GSMTC) failed; skipping");
        }
    }

    /// Blocking wait for a WinRT async op, built from the generated methods
    /// (`Status`/`SetCompleted`/`GetResults`) that exist on the type in every
    /// `windows`/`windows-future` pairing - the convenience `.get()` method's
    /// availability has shifted across crate versions (the v0.5.1-rc1 build
    /// failed on exactly that), and this crate can't be compile-checked from
    /// the Linux dev box.
    fn wait_op<T>(op: windows::Foundation::IAsyncOperation<T>) -> windows::core::Result<T>
    where
        T: windows::core::RuntimeType,
    {
        use windows::Foundation::{AsyncOperationCompletedHandler, AsyncStatus};
        if op.Status()? == AsyncStatus::Started {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            op.SetCompleted(&AsyncOperationCompletedHandler::new(move |_, _| {
                let _ = tx.send(());
                Ok(())
            }))?;
            // A lost signal (handler dropped) would hang forever - bound it.
            let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
        }
        op.GetResults()
    }

    fn try_pause_playing() -> windows::core::Result<Vec<String>> {
        // `.join()` is the blocking wait in windows-future 0.3 (the 0.2 line
        // called it `.get()`); inherent method, no trait import needed.
        let manager = SessionManager::RequestAsync()?.join()?;
        let sessions = manager.GetSessions()?;
        let mut paused = Vec::new();
        for session in sessions {
            let playing = session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .map(|status| status == PlaybackStatus::Playing)
                .unwrap_or(false);
            if !playing {
                continue;
            }
            // TryPauseAsync returns whether the control was accepted; only
            // remember the ones we actually asked to pause and that succeeded.
            let paused_ok = session
                .TryPauseAsync()
                .and_then(|op| op.join())
                .unwrap_or(false);
            if paused_ok {
                if let Ok(id) = session.SourceAppUserModelId() {
                    paused.push(id.to_string_lossy());
                }
            }
        }
        Ok(paused)
    }

    fn try_resume(targets: &[String]) -> windows::core::Result<()> {
        if targets.is_empty() {
            return Ok(());
        }
        let manager = SessionManager::RequestAsync()?.join()?;
        let sessions = manager.GetSessions()?;
        for session in sessions {
            let Ok(id) = session.SourceAppUserModelId() else {
                continue;
            };
            if targets.iter().any(|t| *t == id.to_string_lossy()) {
                // The player may have gone away; ignore per-session failures.
                let _ = session.TryPlayAsync().and_then(|op| op.join());
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    //! Best-effort via `osascript`. Only Music and Spotify, and only when
    //! already running + playing (guarded with `is running` so we never launch
    //! an app). Flakier than MPRIS/GSMTC: other players are simply not covered.

    const APPS: &[&str] = &["Music", "Spotify"];

    pub fn pause_playing() -> Vec<String> {
        let mut paused = Vec::new();
        for app in APPS {
            if pause_one(app) {
                paused.push((*app).to_string());
            }
        }
        paused
    }

    pub fn resume(targets: Vec<String>) {
        for app in targets {
            resume_one(&app);
        }
    }

    /// Pause `app` iff it is already running and playing; returns whether it
    /// was actually paused (so we only remember what we touched).
    fn pause_one(app: &str) -> bool {
        let script = format!(
            r#"if application "{app}" is running then
    tell application "{app}"
        if player state is playing then
            pause
            return "paused"
        end if
    end tell
end if
return """#
        );
        match run_osascript(&script) {
            Ok(out) => out.trim() == "paused",
            Err(error) => {
                tracing::debug!(%error, app, "dictation media auto-pause (osascript) failed");
                false
            }
        }
    }

    fn resume_one(app: &str) {
        let script =
            format!(r#"if application "{app}" is running then tell application "{app}" to play"#);
        if let Err(error) = run_osascript(&script) {
            tracing::debug!(%error, app, "dictation media resume (osascript) failed");
        }
    }

    fn run_osascript(script: &str) -> std::io::Result<String> {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    static LOGGED: AtomicBool = AtomicBool::new(false);

    pub fn pause_playing() -> Vec<String> {
        if !LOGGED.swap(true, Ordering::Relaxed) {
            tracing::debug!("dictation media auto-pause is not implemented on this platform");
        }
        Vec::new()
    }

    pub fn resume(_targets: Vec<String>) {}
}

#[cfg(test)]
mod tests {
    use super::MediaPauseState;

    #[test]
    fn resume_only_touches_what_pause_recorded() {
        let state = MediaPauseState::default();
        // Nothing paused yet: resume drains an empty set.
        assert!(state.take().is_empty());

        state.record(vec!["org.mpris.MediaPlayer2.spotify".into()]);
        let resumed = state.take();
        assert_eq!(resumed, vec!["org.mpris.MediaPlayer2.spotify".to_string()]);

        // Drained: a second resume is a no-op (idempotent), so we can't
        // double-resume or un-pause something the user re-paused after us.
        assert!(state.take().is_empty());
    }

    #[test]
    fn record_dedupes_repeated_pauses() {
        let state = MediaPauseState::default();
        state.record(vec!["a".into(), "b".into()]);
        // A second pause (e.g. a racy double session-start) must not queue the
        // same target twice, or resume would call Play on it redundantly.
        state.record(vec!["b".into(), "c".into()]);
        let mut resumed = state.take();
        resumed.sort();
        assert_eq!(resumed, vec!["a", "b", "c"]);
    }

    #[test]
    fn recording_nothing_keeps_the_set_empty() {
        let state = MediaPauseState::default();
        state.record(vec![]);
        assert!(state.take().is_empty());
    }
}
