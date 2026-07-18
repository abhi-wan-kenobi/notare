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

    // --- Persistent dictation orb, available on every platform since #31
    // --- (macOS reaches parity through this same webview orb instead of its
    // --- unfinished native panel).

    pub fn show_orb(&self) -> Result<(), Error> {
        crate::orb::show()
    }

    pub fn hide_orb(&self) -> Result<(), Error> {
        crate::orb::hide()
    }

    pub async fn start_dictation(
        &self,
        base_url: String,
        model: String,
        output_mode: crate::events::DictationOutputMode,
    ) -> Result<(), Error> {
        crate::session::start(base_url, model, output_mode).await
    }

    pub fn stop_dictation(&self) -> Result<(), Error> {
        crate::session::stop(crate::orb::app_handle()?);
        Ok(())
    }

    pub fn is_dictating(&self) -> Result<bool, Error> {
        Ok(crate::session::is_running(crate::orb::app_handle()?))
    }

    pub async fn type_text(&self, text: String) -> Result<(), Error> {
        tauri::async_runtime::spawn_blocking(move || crate::inject::type_text(&text))
            .await
            .map_err(|e| Error::Inject(format!("injection task panicked: {e}")))?
    }

    /// Copy `text` to the clipboard; with `paste_at_cursor` also synthesize
    /// the platform paste chord (Ctrl+V, or Cmd+V on macOS - see
    /// `inject::send_paste_chord`) into the focused app (batch-mode
    /// delivery).
    pub async fn deliver_text(&self, text: String, paste_at_cursor: bool) -> Result<(), Error> {
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
