#[cfg(target_os = "macos")]
use crate::events::Modifier;
use crate::{
    error::Error,
    events::{HotKey, Options},
};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::stub::Handler;

/// Parse-validate an accelerator string in `tauri-plugin-global-shortcut`
/// syntax ("ctrl+alt+space") without registering anything. Backs the
/// `parse_global_hotkey` command; the settings recorder uses it for inline
/// feedback before committing a new shortcut.
#[cfg(not(target_os = "macos"))]
pub fn parse_global(shortcut: &str) -> Result<(), Error> {
    shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map(|_| ())
        .map_err(|e| Error::InvalidShortcut(format!("{shortcut}: {e}")))
}

/// macOS keeps its native push-to-talk path and never registers these
/// toggle-style accelerators, so every string "parses".
#[cfg(target_os = "macos")]
pub fn parse_global(_shortcut: &str) -> Result<(), Error> {
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
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

    impl Handler {
        /// Toggle-style global hotkeys are the Windows/Linux dictation path;
        /// macOS keeps its native event-tap flow untouched.
        pub fn register_global<R: Runtime>(
            &self,
            _app: AppHandle<R>,
            _shortcut: String,
        ) -> Result<(), Error> {
            Err(Error::Unsupported)
        }

        pub fn unregister_global<R: Runtime>(&self, _app: AppHandle<R>) -> Result<(), Error> {
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use std::sync::Mutex;

    use tauri::{AppHandle, Runtime};
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
    use tauri_specta::Event;

    use super::{Error, HotKey, Options};
    use crate::events::GlobalHotkeyTriggered;

    pub struct Handler {
        /// Currently registered toggle hotkey (Windows/Linux), kept so a
        /// re-register or teardown can unregister the previous binding.
        global: Mutex<Option<Shortcut>>,
    }

    impl Handler {
        pub fn new() -> Self {
            Self {
                global: Mutex::new(None),
            }
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

        /// Toggle-style global hotkey backed by `tauri-plugin-global-shortcut`
        /// (must be registered on the app builder; the desktop app does this
        /// for non-macOS targets). `shortcut` uses the plugin's string syntax,
        /// e.g. `"ctrl+alt+space"`. Fires `GlobalHotkeyTriggered` on key-down.
        pub fn register_global<R: Runtime>(
            &self,
            app: AppHandle<R>,
            shortcut: String,
        ) -> Result<(), Error> {
            let parsed: Shortcut = shortcut
                .parse()
                .map_err(|e| Error::InvalidShortcut(format!("{shortcut}: {e}")))?;

            let mut guard = self.global.lock().unwrap_or_else(|e| e.into_inner());
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

        pub fn unregister_global<R: Runtime>(&self, app: AppHandle<R>) -> Result<(), Error> {
            let mut guard = self.global.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(previous) = guard.take() {
                app.global_shortcut()
                    .unregister(previous)
                    .map_err(|e| Error::GlobalShortcut(e.to_string()))?;
            }
            Ok(())
        }
    }
}
