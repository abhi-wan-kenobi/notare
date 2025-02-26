use once_cell::sync::Lazy;
use std::sync::Mutex;

static PLAYING_SOUNDS: Lazy<
    Mutex<std::collections::HashMap<AppSounds, std::sync::mpsc::Sender<()>>>,
> = Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type, Clone, PartialEq, Eq, Hash)]
pub enum AppSounds {
    StartRecording,
    StopRecording,
    BGM,
}

impl AppSounds {
    pub fn play(&self) {
        self.stop();

        let bytes = self.get_sound_bytes();
        let stop_sender = hypr_audio::AudioOutput::to_speaker(bytes);

        {
            let mut sounds = PLAYING_SOUNDS.lock().unwrap();
            sounds.insert(self.clone(), stop_sender);
        }
    }

    pub fn stop(&self) {
        let mut sounds = PLAYING_SOUNDS.lock().unwrap();
        if let Some(tx) = sounds.remove(self) {
            let _ = tx.send(());
        }
    }

    fn get_sound_bytes(&self) -> &'static [u8] {
        match self {
            AppSounds::StartRecording => include_bytes!("../sounds/start_recording.ogg"),
            AppSounds::StopRecording => include_bytes!("../sounds/stop_recording.ogg"),
            AppSounds::BGM => include_bytes!("../sounds/bgm.mp3"),
        }
    }
}

pub trait SfxPluginExt<R: tauri::Runtime> {
    fn play(&self, sfx: AppSounds);
    fn stop(&self, sfx: AppSounds);
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> SfxPluginExt<R> for T {
    fn play(&self, sfx: AppSounds) {
        sfx.play();
    }

    fn stop(&self, sfx: AppSounds) {
        sfx.stop();
    }
}
