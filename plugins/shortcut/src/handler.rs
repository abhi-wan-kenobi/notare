use crate::{
    error::Error,
    events::{HotKey, Modifier, Options, Permissions},
};

#[cfg(target_os = "macos")]
pub use self::macos::Handler;

#[cfg(not(target_os = "macos"))]
pub use self::stub::Handler;

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::{Arc, Mutex};

    use hypr_dictation_ui_macos as ui;
    use hypr_shortcut_macos as sm;
    use tauri::{AppHandle, Runtime};
    use tauri_specta::Event;
    use tokio::time::{Duration, sleep};

    use super::{Error, HotKey, Modifier, Options, Permissions};
    use crate::events::ShortcutEvent;

    pub struct Handler {
        inner: Arc<Mutex<Inner>>,
    }

    struct Inner {
        processor: Option<Arc<Mutex<sm::HotKeyProcessor>>>,
        tap: Option<sm::EventTap>,
    }

    impl Handler {
        pub fn new<R: Runtime>(app: AppHandle<R>) -> Self {
            let inner = Arc::new(Mutex::new(Inner {
                processor: None,
                tap: None,
            }));
            spawn_permission_watcher(app);
            Self { inner }
        }

        pub fn register<R: Runtime>(
            &self,
            app: AppHandle<R>,
            hotkey: HotKey,
            options: Options,
        ) -> Result<(), Error> {
            if !sm::permission::check_accessibility() {
                sm::permission::prompt_accessibility();
                return Err(Error::AccessibilityDenied);
            }

            if !sm::permission::check_input_monitoring() {
                return Err(Error::InputMonitoringDenied);
            }

            let sm_hotkey = convert_hotkey(&hotkey);
            let sm_options = convert_options(options);

            let processor = Arc::new(Mutex::new({
                let mut p = sm::HotKeyProcessor::new(sm_hotkey);
                p.set_options(sm_options);
                p
            }));

            let cb_processor = processor.clone();
            let tap = sm::EventTap::start(move |event| {
                let mut p = cb_processor.lock().unwrap_or_else(|e| e.into_inner());
                let out = match event {
                    sm::TapEvent::Key(k) => p.process_key(k),
                    sm::TapEvent::MouseClick => p.process_mouse_click(),
                };
                drop(p);
                if let Some(out) = out {
                    let evt = match out {
                        sm::Output::StartRecording => {
                            ui::show();
                            ui::update_state(&ui::DictationState {
                                phase: ui::Phase::Recording,
                                amplitude: 0.0,
                            });
                            ShortcutEvent::Start
                        }
                        sm::Output::StopRecording => {
                            ui::update_state(&ui::DictationState {
                                phase: ui::Phase::Processing,
                                amplitude: 0.0,
                            });
                            ui::hide();
                            ShortcutEvent::Stop
                        }
                        sm::Output::Cancel => {
                            ui::hide();
                            ShortcutEvent::Cancel
                        }
                        sm::Output::Discard => {
                            ui::hide();
                            ShortcutEvent::Discard
                        }
                    };
                    let _ = evt.emit(&app);
                }
            })
            .map_err(|e| Error::TapStart(e.to_string()))?;

            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.tap = Some(tap);
            inner.processor = Some(processor);
            Ok(())
        }

        pub fn unregister(&self) -> Result<(), Error> {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.tap.take();
            inner.processor = None;
            Ok(())
        }

        pub fn check_permissions(&self) -> Permissions {
            Permissions {
                accessibility: sm::permission::check_accessibility(),
                input_monitoring: sm::permission::check_input_monitoring(),
            }
        }

        pub fn request_accessibility_permission(&self) -> Result<bool, Error> {
            Ok(sm::permission::prompt_accessibility())
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

    fn spawn_permission_watcher<R: Runtime>(app: AppHandle<R>) {
        tauri::async_runtime::spawn(async move {
            let mut last = (
                sm::permission::check_accessibility(),
                sm::permission::check_input_monitoring(),
            );
            let _ = ShortcutEvent::PermissionChanged {
                accessibility: last.0,
                input_monitoring: last.1,
            }
            .emit(&app);

            loop {
                sleep(Duration::from_millis(500)).await;
                let current = (
                    sm::permission::check_accessibility(),
                    sm::permission::check_input_monitoring(),
                );
                if current != last {
                    let _ = ShortcutEvent::PermissionChanged {
                        accessibility: current.0,
                        input_monitoring: current.1,
                    }
                    .emit(&app);
                    last = current;
                }
            }
        });
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use tauri::{AppHandle, Runtime};

    use super::{Error, HotKey, Options, Permissions};

    pub struct Handler;

    impl Handler {
        pub fn new<R: Runtime>(_app: AppHandle<R>) -> Self {
            Self
        }

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

        pub fn check_permissions(&self) -> Permissions {
            Permissions {
                accessibility: true,
                input_monitoring: true,
            }
        }

        pub fn request_accessibility_permission(&self) -> Result<bool, Error> {
            Ok(true)
        }
    }
}
