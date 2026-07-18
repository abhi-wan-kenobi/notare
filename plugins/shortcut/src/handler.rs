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

    /// Toggle-style global hotkey backed by `tauri-plugin-global-shortcut`
    /// (must be registered on the app builder - the desktop app does this on
    /// every platform). `shortcut` uses the plugin's string syntax, e.g.
    /// `"ctrl+alt+space"`. Fires `GlobalHotkeyTriggered` on key-down.
    pub fn register_global<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        shortcut: String,
    ) -> Result<(), Error> {
        self.global.register(app, shortcut)
    }

    pub fn unregister_global<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
    ) -> Result<(), Error> {
        self.global.unregister(app)
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
        /// Currently registered toggle hotkey, kept so a re-register or
        /// teardown can unregister the previous binding.
        current: Mutex<Option<Shortcut>>,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                current: Mutex::new(None),
            }
        }

        pub fn register<R: Runtime>(
            &self,
            app: AppHandle<R>,
            shortcut: String,
        ) -> Result<(), Error> {
            let parsed: Shortcut = shortcut
                .parse()
                .map_err(|e| Error::InvalidShortcut(format!("{shortcut}: {e}")))?;

            let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(previous) = guard.take() {
                let _ = app.global_shortcut().unregister(previous);
            }

            let emitted = shortcut.clone();
            app.global_shortcut()
                .on_shortcut(parsed, move |app, _sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = GlobalHotkeyTriggered {
                            shortcut: emitted.clone(),
                        }
                        .emit(app);
                    }
                })
                .map_err(|e| Error::GlobalShortcut(e.to_string()))?;

            *guard = Some(parsed);
            Ok(())
        }

        pub fn unregister<R: Runtime>(&self, app: AppHandle<R>) -> Result<(), Error> {
            let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(previous) = guard.take() {
                app.global_shortcut()
                    .unregister(previous)
                    .map_err(|e| Error::GlobalShortcut(e.to_string()))?;
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
