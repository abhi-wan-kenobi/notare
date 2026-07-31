#[cfg(target_os = "macos")]
use crate::events::Modifier;
use crate::{
    error::Error,
    events::{HotKey, Options},
};

/// Combines the two independent hotkey paths the plugin exposes:
/// - `push_to_talk` - macOS-only native event-tap path (hold-to-record
///   style), driven by `register`/`unregister`. Not wired to any frontend
///   command today.
/// - `global` - toggle-style accelerator backed by
///   `tauri-plugin-global-shortcut`, driven by `register_global`/
///   `unregister_global`. Backs the dictation-orb shortcut on every platform
///   since #31 (macOS previously kept this path `Unsupported` and relied on
///   an unfinished native panel instead).
pub struct Handler {
    push_to_talk: push_to_talk::Handler,
    global: global::Handler,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            push_to_talk: push_to_talk::Handler::new(),
            global: global::Handler::new(),
        }
    }

    /// macOS native push-to-talk hotkey path: `Unsupported` off macOS.
    pub fn register<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        hotkey: HotKey,
        options: Options,
    ) -> Result<(), Error> {
        self.push_to_talk.register(app, hotkey, options)
    }

    pub fn unregister(&self) -> Result<(), Error> {
        self.push_to_talk.unregister()
    }

    /// Keyed global hotkey backed by `tauri-plugin-global-shortcut` (must be
    /// registered on the app builder - the desktop app does this on every
    /// platform). Multiple hotkeys can be live at once, each under its own
    /// `id`; re-registering an `id` replaces its previous binding. `shortcut`
    /// uses the plugin's string syntax, e.g. `"ctrl+alt+space"`. Fires
    /// `GlobalHotkeyTriggered { id, shortcut }` on key-down.
    pub fn register_global<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        id: String,
        shortcut: String,
    ) -> Result<(), Error> {
        self.global.register(app, id, shortcut)
    }

    pub fn unregister_global<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        id: String,
    ) -> Result<(), Error> {
        self.global.unregister(app, id)
    }
}

/// Parse-validate an accelerator string in `tauri-plugin-global-shortcut`
/// syntax ("ctrl+alt+space") without registering anything. Backs the
/// `parse_global_hotkey` command; the settings recorder uses it for inline
/// feedback before committing a new shortcut.
pub fn parse_global(shortcut: &str) -> Result<(), Error> {
    global::parse(shortcut)
}

/// Toggle-style global hotkey (`register_global`/`unregister_global`),
/// backed by `tauri-plugin-global-shortcut` on every platform.
mod global {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use tauri::{AppHandle, Runtime};
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
    use tauri_specta::Event;

    use super::Error;
    use crate::events::GlobalHotkeyTriggered;

    pub fn parse(shortcut: &str) -> Result<(), Error> {
        shortcut
            .parse::<Shortcut>()
            .map(|_| ())
            .map_err(|e| Error::InvalidShortcut(format!("{shortcut}: {e}")))
    }

    pub struct Handler {
        /// Registered hotkeys keyed by caller id, so several can be live at
        /// once (the dictation toggle and paste-last hotkeys). The original
        /// accelerator string rides along so a failed re-register can restore
        /// the previous binding instead of leaving the id unbound.
        current: Mutex<HashMap<String, (Shortcut, String)>>,
    }

    /// Bind `parsed` to the OS-level shortcut, emitting the keyed event on
    /// press. Split out so a failed re-register can re-bind the previous
    /// accelerator with the same closure shape.
    fn bind<R: Runtime>(
        app: &AppHandle<R>,
        id: &str,
        shortcut: &str,
        parsed: Shortcut,
    ) -> Result<(), Error> {
        let emitted_id = id.to_string();
        let emitted_shortcut = shortcut.to_string();
        app.global_shortcut()
            .on_shortcut(parsed, move |app, _sc, event| {
                if event.state() == ShortcutState::Pressed {
                    let _ = GlobalHotkeyTriggered {
                        id: emitted_id.clone(),
                        shortcut: emitted_shortcut.clone(),
                    }
                    .emit(app);
                }
            })
            .map_err(|e| Error::GlobalShortcut(e.to_string()))
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                current: Mutex::new(HashMap::new()),
            }
        }

        pub fn register<R: Runtime>(
            &self,
            app: AppHandle<R>,
            id: String,
            shortcut: String,
        ) -> Result<(), Error> {
            let parsed: Shortcut = shortcut
                .parse()
                .map_err(|e| Error::InvalidShortcut(format!("{shortcut}: {e}")))?;

            let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
            // Replace only this id's previous binding; other ids stay live.
            // The old binding must come off first (re-registering the same
            // accelerator would otherwise collide with itself) - but a failed
            // new registration restores it so the id is never left unbound.
            let previous = guard.remove(&id);
            if let Some((prev, _)) = &previous {
                let _ = app.global_shortcut().unregister(*prev);
            }

            match bind(&app, &id, &shortcut, parsed) {
                Ok(()) => {
                    guard.insert(id, (parsed, shortcut));
                    Ok(())
                }
                Err(error) => {
                    if let Some((prev, prev_shortcut)) = previous {
                        if bind(&app, &id, &prev_shortcut, prev).is_ok() {
                            guard.insert(id, (prev, prev_shortcut));
                        }
                    }
                    Err(error)
                }
            }
        }

        pub fn unregister<R: Runtime>(&self, app: AppHandle<R>, id: String) -> Result<(), Error> {
            let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((previous, shortcut)) = guard.remove(&id) {
                if let Err(e) = app.global_shortcut().unregister(previous) {
                    // The OS still fires this binding - keep tracking it so a
                    // later re-register can still replace it.
                    guard.insert(id, (previous, shortcut));
                    return Err(Error::GlobalShortcut(e.to_string()));
                }
            }
            Ok(())
        }
    }
}

/// Push-to-talk hotkey (`register`/`unregister`): macOS native event-tap
/// path, `Unsupported` everywhere else. Not wired to any frontend command
/// today - the dictation orb uses the `global` toggle path above instead.
#[cfg(target_os = "macos")]
mod push_to_talk {
    use std::{sync::Mutex, time::Duration};

    use hypr_shortcut_macos as sm;
    use tauri::{AppHandle, Runtime};
    use tauri_specta::Event;

    use super::{Error, HotKey, Modifier, Options};
    use crate::events::ShortcutEvent;

    pub struct Handler {
        listener: Mutex<Option<sm::Listener>>,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                listener: Mutex::new(None),
            }
        }

        pub fn register<R: Runtime>(
            &self,
            app: AppHandle<R>,
            hotkey: HotKey,
            options: Options,
        ) -> Result<(), Error> {
            let listener = sm::Listener::start(
                convert_hotkey(&hotkey),
                convert_options(options),
                move |out| {
                    let evt = match out {
                        sm::Output::StartRecording => ShortcutEvent::Pressed,
                        sm::Output::StopRecording => ShortcutEvent::Released,
                        sm::Output::Cancel => ShortcutEvent::Cancelled,
                        sm::Output::Discard => ShortcutEvent::Discarded,
                    };
                    let _ = evt.emit(&app);
                },
            )
            .map_err(|e| Error::TapStart(e.to_string()))?;

            *self.listener.lock().unwrap_or_else(|e| e.into_inner()) = Some(listener);
            Ok(())
        }

        pub fn unregister(&self) -> Result<(), Error> {
            self.listener
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            Ok(())
        }
    }

    fn convert_hotkey(hotkey: &HotKey) -> sm::HotKey {
        let mut modifiers = sm::Modifiers::empty();
        for m in &hotkey.modifiers {
            modifiers.insert(match m {
                Modifier::Command => sm::Modifier::Command,
                Modifier::Option => sm::Modifier::Option,
                Modifier::Shift => sm::Modifier::Shift,
                Modifier::Control => sm::Modifier::Control,
                Modifier::Fn => sm::Modifier::Fn,
            });
        }
        sm::HotKey::new(hotkey.key, modifiers)
    }

    fn convert_options(options: Options) -> sm::Options {
        sm::Options {
            use_double_tap_only: options.use_double_tap_only,
            double_tap_lock_enabled: options.double_tap_lock_enabled,
            minimum_key_time: Duration::from_millis(options.minimum_key_time_ms),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod push_to_talk {
    use tauri::{AppHandle, Runtime};

    use super::{Error, HotKey, Options};

    pub struct Handler;

    impl Handler {
        pub fn new() -> Self {
            Self
        }

        /// macOS push-to-talk hotkey path: not available off macOS.
        pub fn register<R: Runtime>(
            &self,
            _app: AppHandle<R>,
            _hotkey: HotKey,
            _options: Options,
        ) -> Result<(), Error> {
            Err(Error::Unsupported)
        }

        pub fn unregister(&self) -> Result<(), Error> {
            Ok(())
        }
    }
}
