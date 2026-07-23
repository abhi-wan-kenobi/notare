use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hypr_audio_utils::{
    MonoMixdown, decode_vorbis_to_mono_wav_file, decode_vorbis_to_wav_file,
    ogg_has_identical_channels,
};
use ractor::ActorProcessingErr;

use super::into_actor_err;

const FINAL_AUDIO_FILE: &str = "audio.mp3";
const WAV_FILE: &str = "audio.wav";
const OGG_FILE: &str = "audio.ogg";
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

pub(super) struct DiskSink {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    writer_mic: Option<hound::WavWriter<BufWriter<File>>>,
    writer_spk: Option<hound::WavWriter<BufWriter<File>>>,
    wav_path: PathBuf,
    last_flush: Instant,
    is_stereo: bool,
    mono_mixdown: MonoMixdown,
}

pub(super) fn create_disk_sink(session_dir: &Path) -> Result<DiskSink, ActorProcessingErr> {
    let wav_path = session_dir.join(WAV_FILE);
    let ogg_path = session_dir.join(OGG_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);
    let is_stereo = prepare_existing_audio_state(&encoded_path, &ogg_path, &wav_path)?;

    let stereo_spec = hound::WavSpec {
        channels: 2,
        sample_rate: super::super::SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mono_spec = hound::WavSpec {
        channels: 1,
        sample_rate: super::super::SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let writer = if wav_path.exists() {
        hound::WavWriter::append(&wav_path)?
    } else if is_stereo {
        hound::WavWriter::create(&wav_path, stereo_spec)?
    } else {
        hound::WavWriter::create(&wav_path, mono_spec)?
    };

    let (writer_mic, writer_spk) = if is_debug_mode() {
        let mic_path = session_dir.join("audio_mic.wav");
        let spk_path = session_dir.join("audio_spk.wav");

        let mic_writer = if mic_path.exists() {
            hound::WavWriter::append(&mic_path)?
        } else {
            hound::WavWriter::create(&mic_path, mono_spec)?
        };

        let spk_writer = if spk_path.exists() {
            hound::WavWriter::append(&spk_path)?
        } else {
            hound::WavWriter::create(&spk_path, mono_spec)?
        };

        (Some(mic_writer), Some(spk_writer))
    } else {
        (None, None)
    };

    Ok(DiskSink {
        writer: Some(writer),
        writer_mic,
        writer_spk,
        wav_path,
        last_flush: Instant::now(),
        is_stereo,
        mono_mixdown: MonoMixdown::new(super::super::SAMPLE_RATE),
    })
}

pub(super) fn write_single(sink: &mut DiskSink, samples: &[f32]) -> Result<(), ActorProcessingErr> {
    if let Some(writer) = sink.writer.as_mut() {
        if sink.is_stereo {
            write_mono_as_stereo(writer, samples)?;
        } else {
            write_mono_samples(writer, samples)?;
        }
    }

    flush_if_due(sink)?;
    Ok(())
}

pub(super) fn write_dual(
    sink: &mut DiskSink,
    mic: &[f32],
    spk: &[f32],
) -> Result<(), ActorProcessingErr> {
    if let Some(writer) = sink.writer.as_mut() {
        if sink.is_stereo {
            write_interleaved_stereo(writer, mic, spk)?;
        } else {
            let mixed = sink.mono_mixdown.mix(mic, spk);
            write_mono_samples(writer, &mixed)?;
        }
    }

    if let Some(writer_mic) = sink.writer_mic.as_mut() {
        write_mono_samples(writer_mic, mic)?;
    }

    if let Some(writer_spk) = sink.writer_spk.as_mut() {
        write_mono_samples(writer_spk, spk)?;
    }

    flush_if_due(sink)?;
    Ok(())
}

pub(super) fn finalize_disk_sink(sink: &mut DiskSink) -> Result<(), ActorProcessingErr> {
    finalize_writer(&mut sink.writer, Some(&sink.wav_path))?;
    finalize_writer(&mut sink.writer_mic, None)?;
    finalize_writer(&mut sink.writer_spk, None)?;

    if sink.wav_path.exists() {
        let encoded_path = sink.wav_path.with_extension("mp3");
        match hypr_mp3::encode_wav(&sink.wav_path, &encoded_path) {
            Ok(()) => {
                sync_file(&encoded_path);
                sync_dir(&encoded_path);
                std::fs::remove_file(&sink.wav_path)?;
                sync_dir(&sink.wav_path);
            }
            Err(error) => {
                tracing::error!("Encoding to mp3 failed, keeping WAV: {}", error);
                sync_file(&sink.wav_path);
                sync_dir(&sink.wav_path);
            }
        }
    }

    Ok(())
}

/// Finalize an `audio.wav` left behind by a hard process kill (supervisor
/// meltdown, OOM, `kill -9`, power loss) when NO recorder actor is alive to run
/// `finalize_disk_sink`. Because the live recorder flushes the hound writer
/// every second and hound's `flush()` rewrites the RIFF/`data` sizes, the
/// orphaned `audio.wav` is a valid, readable WAV up to the last flush (≤ ~1s of
/// tail lost — "≤ the last buffer", not the session). Encode it to the final
/// `audio.mp3`, fsync, and drop the WAV — mirroring `finalize_disk_sink`'s
/// encode tail but without an in-memory writer.
///
/// Returns `Ok(true)` when a recording was recovered, `Ok(false)` when there is
/// nothing to do (already finalized, or no WAV present). Never touches a dir
/// that already has `audio.mp3` (that recording finalized cleanly).
pub(super) fn finalize_orphaned_wav(session_dir: &Path) -> Result<bool, ActorProcessingErr> {
    let wav_path = session_dir.join(WAV_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);

    if encoded_path.exists() || !wav_path.exists() {
        return Ok(false);
    }

    match hypr_mp3::encode_wav(&wav_path, &encoded_path) {
        Ok(()) => {
            sync_file(&encoded_path);
            sync_dir(&encoded_path);
            std::fs::remove_file(&wav_path)?;
            sync_dir(&wav_path);
            Ok(true)
        }
        Err(error) => {
            // Keep the WAV for manual recovery rather than deleting evidence.
            tracing::error!(
                dir = %session_dir.display(),
                "orphan recovery encode failed, keeping wav: {}",
                error
            );
            Ok(false)
        }
    }
}

fn prepare_existing_audio_state(
    encoded_path: &Path,
    ogg_path: &Path,
    wav_path: &Path,
) -> Result<bool, ActorProcessingErr> {
    if encoded_path.exists() {
        decode_mp3_to_wav(encoded_path, wav_path)?;
        std::fs::remove_file(encoded_path)?;
        return Ok(wav_is_stereo(wav_path)?);
    }

    if ogg_path.exists() {
        let has_identical = ogg_has_identical_channels(ogg_path).map_err(into_actor_err)?;
        if has_identical {
            decode_vorbis_to_mono_wav_file(ogg_path, wav_path).map_err(into_actor_err)?;
        } else {
            decode_vorbis_to_wav_file(ogg_path, wav_path).map_err(into_actor_err)?;
        }
        std::fs::remove_file(ogg_path)?;
        return Ok(!has_identical);
    }

    if wav_path.exists() {
        return Ok(wav_is_stereo(wav_path)?);
    }

    Ok(true)
}

fn decode_mp3_to_wav(encoded_path: &Path, wav_path: &Path) -> Result<(), ActorProcessingErr> {
    let tmp_path = wav_path.with_extension("wav.tmp");
    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    hypr_mp3::decode_to_wav(encoded_path, &tmp_path).map_err(into_actor_err)?;

    if wav_path.exists() {
        std::fs::remove_file(wav_path)?;
    }
    std::fs::rename(tmp_path, wav_path)?;
    Ok(())
}

fn wav_is_stereo(wav_path: &Path) -> Result<bool, hound::Error> {
    let reader = hound::WavReader::open(wav_path)?;
    Ok(reader.spec().channels == 2)
}

fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
        || std::env::var("LISTENER_DEBUG")
            .map(|value| !value.is_empty() && value != "0" && value != "false")
            .unwrap_or(false)
}

fn flush_if_due(sink: &mut DiskSink) -> Result<(), hound::Error> {
    if sink.last_flush.elapsed() < FLUSH_INTERVAL {
        return Ok(());
    }

    flush_all(sink)
}

fn flush_all(sink: &mut DiskSink) -> Result<(), hound::Error> {
    if let Some(writer) = sink.writer.as_mut() {
        writer.flush()?;
    }
    if let Some(writer_mic) = sink.writer_mic.as_mut() {
        writer_mic.flush()?;
    }
    if let Some(writer_spk) = sink.writer_spk.as_mut() {
        writer_spk.flush()?;
    }
    sink.last_flush = Instant::now();
    Ok(())
}

fn write_mono_samples(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    Ok(())
}

fn write_mono_as_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(*sample)?;
        writer.write_sample(*sample)?;
    }
    Ok(())
}

fn write_interleaved_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    mic: &[f32],
    spk: &[f32],
) -> Result<(), hound::Error> {
    let frames = mic.len().max(spk.len());
    for i in 0..frames {
        writer.write_sample(mic.get(i).copied().unwrap_or(0.0))?;
        writer.write_sample(spk.get(i).copied().unwrap_or(0.0))?;
    }
    Ok(())
}

fn finalize_writer(
    writer: &mut Option<hound::WavWriter<BufWriter<File>>>,
    path: Option<&Path>,
) -> Result<(), hound::Error> {
    if let Some(mut writer) = writer.take() {
        writer.flush()?;
        writer.finalize()?;

        if let Some(path) = path {
            sync_file(path);
        }
    }
    Ok(())
}

fn sync_file(path: &Path) {
    if let Ok(file) = File::open(path) {
        let _ = file.sync_all();
    }
}

fn sync_dir(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::actors::SAMPLE_RATE;

    use super::*;

    #[test]
    fn create_disk_sink_decodes_existing_mp3_to_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(
            hypr_data::english_1::AUDIO_MP3_PATH,
            session_dir.join(FINAL_AUDIO_FILE),
        )
        .unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }

    #[test]
    fn create_disk_sink_prefers_existing_mp3_over_stale_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        let encoded_path = session_dir.join(FINAL_AUDIO_FILE);
        let wav_path = session_dir.join(WAV_FILE);
        std::fs::copy(hypr_data::english_1::AUDIO_MP3_PATH, &encoded_path).unwrap();
        write_test_wav(&wav_path, 128);
        let original_frames = decoded_frame_count(&encoded_path);

        let mut sink = create_disk_sink(&session_dir).unwrap();
        write_single(&mut sink, &vec![0.0; SAMPLE_RATE as usize]).unwrap();
        finalize_disk_sink(&mut sink).unwrap();

        assert!(!wav_path.exists());
        assert!(encoded_path.exists());
        assert!(decoded_frame_count(&encoded_path) > original_frames);
    }

    #[test]
    fn create_disk_sink_keeps_legacy_wav_for_append() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(hypr_data::english_1::AUDIO_PATH, session_dir.join(WAV_FILE)).unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }

    #[test]
    fn finalize_orphaned_wav_recovers_unfinalized_wav() {
        // Simulate a hard kill mid-recording: write samples, FLUSH (as the live
        // recorder does every second), then drop the writer WITHOUT finalize —
        // exactly the on-disk state after `kill -9`. hound's flush keeps the
        // header valid, so the WAV is readable and recoverable.
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("11111111-1111-1111-1111-111111111111");
        std::fs::create_dir_all(&session_dir).unwrap();
        let wav_path = session_dir.join(WAV_FILE);
        let frames = SAMPLE_RATE as usize; // 1 second

        write_flushed_but_unfinalized_wav(&wav_path, frames);
        // Header is valid after flush even though finalize() never ran.
        assert!(hound::WavReader::open(&wav_path).is_ok());

        let recovered = finalize_orphaned_wav(&session_dir).unwrap();
        assert!(recovered, "orphaned wav should be recovered");

        let mp3_path = session_dir.join(FINAL_AUDIO_FILE);
        assert!(mp3_path.exists(), "recovered mp3 must exist");
        assert!(!wav_path.exists(), "wav dropped after successful encode");
        // The recovered audio must contain (almost) all captured frames — we
        // lose at most the sub-second tail written after the last flush.
        let recovered_frames = decoded_frame_count(&mp3_path);
        assert!(
            recovered_frames as f64 >= frames as f64 * 0.8,
            "recovered {recovered_frames} frames, expected ~{frames}"
        );
    }

    #[test]
    fn finalize_orphaned_wav_skips_finalized_session() {
        // A session that finalized cleanly (audio.mp3 present) must never be
        // touched, even if a stale wav also lingers.
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        let mp3_path = session_dir.join(FINAL_AUDIO_FILE);
        std::fs::copy(hypr_data::english_1::AUDIO_MP3_PATH, &mp3_path).unwrap();
        write_test_wav(&session_dir.join(WAV_FILE), 128);

        let recovered = finalize_orphaned_wav(&session_dir).unwrap();
        assert!(!recovered, "finalized session must be skipped");
        assert!(
            session_dir.join(WAV_FILE).exists(),
            "stale wav left untouched"
        );
    }

    #[test]
    fn finalize_orphaned_wav_noop_without_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        assert!(!finalize_orphaned_wav(&session_dir).unwrap());
    }

    fn decoded_frame_count(path: &Path) -> usize {
        use hypr_audio_utils::Source;

        let source = hypr_audio_utils::source_from_path(path).unwrap();
        let channels = u16::from(source.channels()).max(1) as usize;
        source.count() / channels
    }

    fn write_flushed_but_unfinalized_wav(path: &Path, frames: usize) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            // Non-trivial signal so the mp3 encoder retains real content.
            let t = i as f32 / SAMPLE_RATE as f32;
            writer
                .write_sample((t * 440.0 * std::f32::consts::TAU).sin() * 0.25)
                .unwrap();
        }
        writer.flush().unwrap();
        // Drop WITHOUT finalize() — the kill -9 state.
        std::mem::drop(writer);
    }

    fn write_test_wav(path: &Path, frames: usize) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..frames {
            writer.write_sample(0.0f32).unwrap();
        }
        writer.finalize().unwrap();
    }
}
