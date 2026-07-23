mod disk;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef};

pub enum RecMsg {
    AudioSingle(Arc<[f32]>),
    AudioDual(Arc<[f32]>, Arc<[f32]>),
}

pub struct RecArgs {
    pub app_dir: PathBuf,
    pub session_id: String,
}

pub struct RecState {
    sink: RecorderSink,
}

enum RecorderSink {
    Disk(disk::DiskSink),
}

pub struct RecorderActor;

impl Default for RecorderActor {
    fn default() -> Self {
        Self::new()
    }
}

impl RecorderActor {
    pub fn new() -> Self {
        Self
    }

    pub fn name() -> ActorName {
        "recorder_actor".into()
    }
}

#[ractor::async_trait]
impl Actor for RecorderActor {
    type Msg = RecMsg;
    type State = RecState;
    type Arguments = RecArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_dir = find_session_dir(&args.app_dir, &args.session_id);
        std::fs::create_dir_all(&session_dir)?;

        Ok(RecState {
            sink: RecorderSink::Disk(disk::create_disk_sink(&session_dir)?),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match (&mut st.sink, msg) {
            (RecorderSink::Disk(sink), RecMsg::AudioSingle(samples)) => {
                disk::write_single(sink, &samples)?;
            }
            (RecorderSink::Disk(sink), RecMsg::AudioDual(mic, spk)) => {
                disk::write_dual(sink, &mic, &spk)?;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match &mut st.sink {
            RecorderSink::Disk(sink) => {
                disk::finalize_disk_sink(sink)?;
            }
        }

        Ok(())
    }
}

pub fn find_session_dir(sessions_base: &Path, session_id: &str) -> PathBuf {
    if let Some(found) = find_session_dir_recursive(sessions_base, session_id) {
        return found;
    }
    sessions_base.join(session_id)
}

pub fn resolve_final_audio_path(sessions_base: &Path, session_id: &str) -> Option<PathBuf> {
    let session_dir = find_session_dir(sessions_base, session_id);
    let mp3_path = session_dir.join("audio.mp3");
    if mp3_path.exists() {
        return Some(mp3_path);
    }

    let wav_path = session_dir.join("audio.wav");
    if wav_path.exists() {
        return Some(wav_path);
    }

    let ogg_path = session_dir.join("audio.ogg");
    if ogg_path.exists() {
        return Some(ogg_path);
    }

    None
}

/// Recover recordings orphaned by a hard process kill: walk the sessions tree
/// and finalize any session dir that has a flushed `audio.wav` but no
/// `audio.mp3` (encode → mp3, fsync, drop the WAV). Safe to call once at
/// startup, BEFORE any new capture session begins — at that point every
/// `audio.wav` on disk is genuinely orphaned (the actor tree that owned it died
/// with the previous process). Returns the number of recordings recovered.
///
/// This is the durability backstop: it closes the gap regardless of WHICH exit
/// path fired (graceful finalize-on-exit, supervisor meltdown, dock/updater
/// restart, `kill -9`, OOM), so the finalize-on-exit fast paths become an
/// optimization rather than a correctness requirement.
pub fn recover_orphaned_recordings(sessions_base: &Path) -> usize {
    let mut recovered = 0;
    recover_orphans_walk(sessions_base, 0, &mut recovered);
    if recovered > 0 {
        tracing::info!(recovered, "recovered_orphaned_recordings");
    }
    recovered
}

fn recover_orphans_walk(dir: &Path, depth: usize, recovered: &mut usize) {
    // Session dirs are UUID-named and may nest under non-UUID container folders
    // (mirrors find_session_dir_recursive); bound the walk defensively.
    const MAX_DEPTH: usize = 8;
    if depth > MAX_DEPTH {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        match disk::finalize_orphaned_wav(&path) {
            Ok(true) => {
                *recovered += 1;
                tracing::info!(dir = %path.display(), "recovered_orphaned_recording");
            }
            Ok(false) => {}
            Err(error) => {
                tracing::error!(?error, dir = %path.display(), "orphan_recovery_failed");
            }
        }

        recover_orphans_walk(&path, depth + 1, recovered);
    }
}

fn find_session_dir_recursive(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path.file_name()?.to_str()?;

        if name == session_id {
            return Some(path);
        }

        if uuid::Uuid::try_parse(name).is_err()
            && let Some(found) = find_session_dir_recursive(&path, session_id)
        {
            return Some(found);
        }
    }

    None
}

fn into_actor_err<E>(err: E) -> ActorProcessingErr
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(err)
}

#[cfg(test)]
mod recovery_tests {
    use std::path::Path;

    use tempfile::tempdir;

    use crate::actors::SAMPLE_RATE;

    use super::*;

    // Writes the on-disk state a `kill -9` leaves behind mid-recording: a WAV
    // flushed by the live recorder (valid header) but never finalize()'d.
    fn write_orphan_wav(session_dir: &Path, frames: usize) {
        std::fs::create_dir_all(session_dir).unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(session_dir.join("audio.wav"), spec).unwrap();
        for i in 0..frames {
            let t = i as f32 / SAMPLE_RATE as f32;
            writer
                .write_sample((t * 330.0 * std::f32::consts::TAU).sin() * 0.25)
                .unwrap();
        }
        writer.flush().unwrap();
        std::mem::drop(writer); // no finalize() — the crash state
    }

    #[test]
    fn recover_orphaned_recordings_finalizes_nested_orphans() {
        // Simulates relaunch after a hard kill: two orphaned sessions (one
        // nested under a non-UUID container dir, as real vaults nest them), one
        // already-finalized session that must be left alone.
        let dir = tempdir().unwrap();
        let sessions_base = dir.path().join("sessions");

        let orphan_a = sessions_base.join("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let orphan_b = sessions_base
            .join("2026-07")
            .join("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        write_orphan_wav(&orphan_a, SAMPLE_RATE as usize);
        write_orphan_wav(&orphan_b, SAMPLE_RATE as usize / 2);

        let finalized = sessions_base.join("cccccccc-cccc-cccc-cccc-cccccccccccc");
        std::fs::create_dir_all(&finalized).unwrap();
        std::fs::copy(
            hypr_data::english_1::AUDIO_MP3_PATH,
            finalized.join("audio.mp3"),
        )
        .unwrap();

        let recovered = recover_orphaned_recordings(&sessions_base);

        assert_eq!(recovered, 2, "both orphaned recordings recovered");
        assert!(orphan_a.join("audio.mp3").exists());
        assert!(!orphan_a.join("audio.wav").exists());
        assert!(orphan_b.join("audio.mp3").exists());
        assert!(!orphan_b.join("audio.wav").exists());
        // The clean session is untouched (no wav was ever there).
        assert!(finalized.join("audio.mp3").exists());

        // Idempotent: a second pass recovers nothing.
        assert_eq!(recover_orphaned_recordings(&sessions_base), 0);
    }
}
