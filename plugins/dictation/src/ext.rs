use crate::{error::Error, events::Phase, handler::Handler};

pub struct Dictation<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Dictation<'a, R, M> {
    pub fn show(&self) -> Result<(), Error> {
        self.manager.state::<Handler>().show()
    }

    pub fn hide(&self) -> Result<(), Error> {
        self.manager.state::<Handler>().hide()
    }

    pub fn set_phase(&self, phase: Phase) -> Result<(), Error> {
        self.manager.state::<Handler>().set_phase(phase)
    }

    pub fn update_amplitude(&self, amplitude: f32) -> Result<(), Error> {
        self.manager.state::<Handler>().update_amplitude(amplitude)
    }

    // --- Persistent dictation orb (Windows/Linux). macOS keeps its native
    // --- panel path untouched; these return `Unsupported` there.

    pub fn show_orb(&self) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            crate::orb::show()
        }
        #[cfg(target_os = "macos")]
        {
            Err(Error::Unsupported)
        }
    }

    pub fn hide_orb(&self) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            crate::orb::hide()
        }
        #[cfg(target_os = "macos")]
        {
            Err(Error::Unsupported)
        }
    }

    pub async fn start_dictation(
        &self,
        base_url: String,
        model: String,
        output_mode: crate::events::DictationOutputMode,
    ) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            crate::session::start(base_url, model, output_mode).await
        }
        #[cfg(target_os = "macos")]
        {
            let _ = (base_url, model, output_mode);
            Err(Error::Unsupported)
        }
    }

    pub fn stop_dictation(&self) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            crate::session::stop(crate::orb::app_handle()?);
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            Err(Error::Unsupported)
        }
    }

    pub fn is_dictating(&self) -> Result<bool, Error> {
        #[cfg(not(target_os = "macos"))]
        {
            Ok(crate::session::is_running(crate::orb::app_handle()?))
        }
        #[cfg(target_os = "macos")]
        {
            Ok(false)
        }
    }

    pub async fn type_text(&self, text: String) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            tauri::async_runtime::spawn_blocking(move || crate::inject::type_text(&text))
                .await
                .map_err(|e| Error::Inject(format!("injection task panicked: {e}")))?
        }
        #[cfg(target_os = "macos")]
        {
            let _ = text;
            Err(Error::Unsupported)
        }
    }

    /// Copy `text` to the clipboard; with `paste_at_cursor` also synthesize
    /// Ctrl+V into the focused app (batch-mode delivery).
    pub async fn deliver_text(&self, text: String, paste_at_cursor: bool) -> Result<(), Error> {
        #[cfg(not(target_os = "macos"))]
        {
            tauri::async_runtime::spawn_blocking(move || {
                if paste_at_cursor {
                    crate::inject::paste_text(&text)
                } else {
                    crate::inject::copy_text(&text)
                }
            })
            .await
            .map_err(|e| Error::Inject(format!("delivery task panicked: {e}")))?
        }
        #[cfg(target_os = "macos")]
        {
            let _ = (text, paste_at_cursor);
            Err(Error::Unsupported)
        }
    }

    /// Deterministic transcript cleanup (`clean.rs`). Pure - available on
    /// every platform.
    pub fn clean_text(&self, text: &str) -> String {
        crate::clean::clean_transcript(text)
    }
}

pub trait DictationPluginExt<R: tauri::Runtime> {
    fn dictation(&self) -> Dictation<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> DictationPluginExt<R> for T {
    fn dictation(&self) -> Dictation<'_, R, Self>
    where
        Self: Sized,
    {
        Dictation {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
