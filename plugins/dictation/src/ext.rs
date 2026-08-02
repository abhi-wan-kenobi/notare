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

    /// Enable/disable the warm-mic holder (Lane B2). Enabling spawns (or leaves
    /// running) a background task that keeps a mic capture stream open and
    /// draining while idle; disabling drops it, releasing the device. The
    /// opener is built from the managed audio provider so the holder can
    /// re-open after a device switch.
    pub fn set_warm_mic(&self, enabled: bool, seq: u32) -> Result<(), Error> {
        let warm = self.manager.state::<crate::warm::WarmMicState>();
        // Last-writer-wins by sequence, enforced INSIDE the state's lock so
        // the check and the apply are atomic - a stale enable landing after a
        // newer disable can never silently re-open the mic (privacy-relevant:
        // the whole point of OFF-by-default).
        if enabled {
            let audio = self
                .manager
                .state::<std::sync::Arc<dyn hypr_audio::AudioProvider>>()
                .inner()
                .clone();
            let chunk_size = hypr_audio_utils::chunk_size_for_stt(crate::session::SAMPLE_RATE);
            let opener = crate::warm::make_opener(audio, crate::session::SAMPLE_RATE, chunk_size);
            warm.enable(opener, seq);
        } else {
            warm.disable(seq);
        }
        Ok(())
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

    /// Pause whatever media is currently playing (see `media.rs`), remembering
    /// what we paused so `resume_media` only un-pauses those exact players.
    /// Returns whether anything was paused. Best-effort: any backend error
    /// degrades to `false`. The platform work runs on the blocking pool so it
    /// never blocks the async executor (or delays the mic).
    pub async fn pause_media(&self) -> bool {
        let state = self.manager.state::<crate::media::MediaPauseState>();
        // Hold the op lock across enumerate+pause+record: a resume racing
        // into the gap before record() would drain an empty list and strand
        // the media paused.
        let _op = state.lock_op().await;
        let paused = tauri::async_runtime::spawn_blocking(crate::media::pause_playing)
            .await
            .unwrap_or_default();
        let any = !paused.is_empty();
        state.record(paused);
        any
    }

    /// Resume only the media WE paused. A no-op if nothing was paused (or it was
    /// already resumed), so it's safe to call redundantly.
    pub async fn resume_media(&self) {
        let state = self.manager.state::<crate::media::MediaPauseState>();
        let _op = state.lock_op().await;
        let targets = state.take();
        if targets.is_empty() {
            return;
        }
        let _ = tauri::async_runtime::spawn_blocking(move || crate::media::resume(targets)).await;
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
