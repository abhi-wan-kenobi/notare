use std::fs::{File, copy, remove_file, rename, write};
use std::io::ErrorKind;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use hypr_audio_utils::Source;
use hypr_resampler::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::error::{AudioImportError, AudioProcessingError};

const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
const AUDIO_FORMATS: [&str; 3] = ["audio.mp3", "audio.wav", "audio.ogg"];
const RESAMPLE_CHUNK_SIZE: usize = 1024;
const MONO_ENCODE_CHUNK_SIZE: usize = 4096;

pub fn exists(session_dir: &Path) -> std::io::Result<bool> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .try_fold(false, |acc, path| {
            std::fs::exists(&path).map(|exists| acc || exists)
        })
}

pub fn delete(session_dir: &Path) -> std::io::Result<()> {
    for format in AUDIO_FORMATS {
        let path = session_dir.join(format);
        if std::fs::exists(&path).unwrap_or(false) {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn path(session_dir: &Path) -> Option<PathBuf> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .find(|path| path.exists())
}

pub fn import_to_session(
    session_dir: &Path,
    source_path: &Path,
) -> Result<PathBuf, AudioImportError> {
    std::fs::create_dir_all(session_dir)?;

    let target_path = session_dir.join("audio.mp3");
    let tmp_path = session_dir.join("audio.mp3.tmp");

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    match import_audio(source_path, &tmp_path, &target_path) {
        Ok(final_path) => Ok(final_path),
        Err(error) => {
            if tmp_path.exists() {
                let _ = std::fs::remove_file(&tmp_path);
            }
            Err(error.into())
        }
    }
}

pub fn import_audio(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, AudioProcessingError> {
    let mp3_bytes = decode_to_mp3(source_path)?;
    write(tmp_path, &mp3_bytes)?;
    atomic_move(tmp_path, target_path)?;
    Ok(target_path.to_path_buf())
}

pub fn decode_to_mp3(source_path: &Path) -> Result<Vec<u8>, AudioProcessingError> {
    let _rodio_err = match decode_to_mp3_with_rodio(source_path) {
        Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
        Ok(_) => None,
        Err(e) => Some(e),
    };

    #[cfg(target_os = "macos")]
    {
        let wav_path = hypr_afconvert::to_wav(source_path)
            .map_err(|e| AudioProcessingError::AfconvertFailed(e.to_string()))?;
        let result = decode_to_mp3_with_rodio(&wav_path).and_then(non_empty_mp3_bytes);
        let _ = std::fs::remove_file(&wav_path);
        return result;
    }

    #[cfg(not(target_os = "macos"))]
    match _rodio_err {
        Some(e) => Err(e),
        None => Err(AudioProcessingError::EmptyInput),
    }
}

fn non_empty_mp3_bytes(bytes: Vec<u8>) -> Result<Vec<u8>, AudioProcessingError> {
    if bytes.is_empty() {
        Err(AudioProcessingError::EmptyInput)
    } else {
        Ok(bytes)
    }
}

fn decode_to_mp3_with_rodio(path: &Path) -> Result<Vec<u8>, AudioProcessingError> {
    let file = File::open(path)?;
    let decoder = rodio::Decoder::try_from(file)?;
    encode_source_to_mp3(decoder)
}

fn encode_source_to_mp3<S>(source: S) -> Result<Vec<u8>, AudioProcessingError>
where
    S: Source<Item = f32>,
{
    let source_rate: u32 = source.sample_rate().into();
    let channel_count_raw: u16 = source.channels().into();
    let channel_count_raw = channel_count_raw.max(1);
    let channel_count_u8 = u8::try_from(channel_count_raw).map_err(|_| {
        AudioProcessingError::UnsupportedChannelCount {
            count: channel_count_raw,
        }
    })?;
    let channel_count =
        NonZeroU8::new(channel_count_u8).ok_or(AudioProcessingError::InvalidChannelCount)?;

    let mp3_err = |e: hypr_mp3::Error| AudioProcessingError::Mp3Encode(e.to_string());
    let mut encoder = hypr_mp3::MonoStreamEncoder::new(TARGET_SAMPLE_RATE_HZ).map_err(mp3_err)?;
    let mut mp3_buffer = Vec::new();
    let channel_count = usize::from(channel_count.get());
    let needs_resample = source_rate != TARGET_SAMPLE_RATE_HZ;
    let mut saw_input = false;

    if needs_resample {
        let mut resampler = create_mono_resampler(source_rate)?;
        let mut input_chunk = vec![Vec::with_capacity(RESAMPLE_CHUNK_SIZE)];
        let mut output_chunk = vec![vec![0.0; resampler.output_frames_max()]];
        let mut frames_to_trim = resampler.output_delay();
        let mut expected_output_frames = 0usize;
        let mut written_output_frames = 0usize;

        for mono_frame in mono_frames(source, channel_count) {
            saw_input = true;
            expected_output_frames += 1;
            input_chunk[0].push(mono_frame);

            if input_chunk[0].len() < resampler.input_frames_next() {
                continue;
            }

            encode_resampler_chunk(
                &mut resampler,
                &mut input_chunk,
                &mut output_chunk,
                &mut encoder,
                &mut mp3_buffer,
                &mut frames_to_trim,
                &mut written_output_frames,
                mp3_err,
                None,
            )?;
        }

        if !saw_input {
            return Ok(Vec::new());
        }

        let expected_output_frames = (expected_output_frames as f64 * TARGET_SAMPLE_RATE_HZ as f64
            / source_rate as f64)
            .ceil() as usize;

        if !input_chunk[0].is_empty() {
            let partial_len = input_chunk[0].len();
            encode_resampler_chunk(
                &mut resampler,
                &mut input_chunk,
                &mut output_chunk,
                &mut encoder,
                &mut mp3_buffer,
                &mut frames_to_trim,
                &mut written_output_frames,
                mp3_err,
                Some(partial_len),
            )?;
        }

        while written_output_frames < expected_output_frames {
            encode_resampler_chunk(
                &mut resampler,
                &mut input_chunk,
                &mut output_chunk,
                &mut encoder,
                &mut mp3_buffer,
                &mut frames_to_trim,
                &mut written_output_frames,
                mp3_err,
                Some(0),
            )?;
        }
    } else {
        let mut mono_chunk = Vec::with_capacity(MONO_ENCODE_CHUNK_SIZE);

        for mono_frame in mono_frames(source, channel_count) {
            saw_input = true;
            mono_chunk.push(mono_frame);

            if mono_chunk.len() < MONO_ENCODE_CHUNK_SIZE {
                continue;
            }

            encoder
                .encode_f32(&mono_chunk, &mut mp3_buffer)
                .map_err(mp3_err)?;
            mono_chunk.clear();
        }

        if !saw_input {
            return Ok(Vec::new());
        }

        if !mono_chunk.is_empty() {
            encoder
                .encode_f32(&mono_chunk, &mut mp3_buffer)
                .map_err(mp3_err)?;
        }
    }

    encoder.flush(&mut mp3_buffer).map_err(mp3_err)?;

    Ok(mp3_buffer)
}

fn create_mono_resampler(source_rate: u32) -> Result<Async<f32>, AudioProcessingError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    Ok(Async::<f32>::new_sinc(
        TARGET_SAMPLE_RATE_HZ as f64 / source_rate as f64,
        2.0,
        &params,
        RESAMPLE_CHUNK_SIZE,
        1,
        FixedAsync::Input,
    )
    .map_err(hypr_resampler::Error::from)?)
}

fn mono_frames<S>(mut source: S, channel_count: usize) -> impl Iterator<Item = f32>
where
    S: Source<Item = f32>,
{
    std::iter::from_fn(move || {
        let first = source.next()?;
        let mut sum = first;
        let mut frame_len = 1usize;

        while frame_len < channel_count {
            let Some(sample) = source.next() else {
                break;
            };
            sum += sample;
            frame_len += 1;
        }

        Some(sum / frame_len as f32)
    })
}

fn encode_resampler_chunk(
    resampler: &mut Async<f32>,
    input_chunk: &mut [Vec<f32>],
    output_chunk: &mut [Vec<f32>],
    encoder: &mut hypr_mp3::MonoStreamEncoder,
    mp3_buffer: &mut Vec<u8>,
    frames_to_trim: &mut usize,
    written_output_frames: &mut usize,
    mp3_err: impl Fn(hypr_mp3::Error) -> AudioProcessingError,
    partial_len: Option<usize>,
) -> Result<(), AudioProcessingError> {
    let frames_needed = resampler.input_frames_next();
    if input_chunk[0].len() < frames_needed {
        input_chunk[0].resize(frames_needed, 0.0);
    }

    let frames_in = input_chunk[0].len();
    let input_adapter =
        SequentialSliceOfVecs::new(input_chunk, 1, frames_in).expect("input adapter");
    let frames_out = output_chunk[0].len();
    let mut output_adapter =
        SequentialSliceOfVecs::new_mut(output_chunk, 1, frames_out).expect("output adapter");
    let indexing = partial_len.map(|partial_len| Indexing {
        input_offset: 0,
        output_offset: 0,
        partial_len: Some(partial_len),
        active_channels_mask: None,
    });
    let (_, produced_frames) = resampler
        .process_into_buffer(&input_adapter, &mut output_adapter, indexing.as_ref())
        .map_err(hypr_resampler::Error::from)?;
    input_chunk[0].clear();

    if produced_frames == 0 {
        return Ok(());
    }

    let trim = (*frames_to_trim).min(produced_frames);
    *frames_to_trim -= trim;

    let encoded_frames = &output_chunk[0][trim..produced_frames];
    if !encoded_frames.is_empty() {
        encoder
            .encode_f32(encoded_frames, mp3_buffer)
            .map_err(mp3_err)?;
        *written_output_frames += encoded_frames.len();
    }

    Ok(())
}

fn atomic_move(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) => {
            #[cfg(unix)]
            let is_cross_device = err.raw_os_error() == Some(18);
            #[cfg(not(unix))]
            let is_cross_device = false;

            if is_cross_device {
                copy(from, to)?;
                remove_file(from)?;
                Ok(())
            } else if err.kind() == ErrorKind::AlreadyExists {
                remove_file(to)?;
                match rename(from, to) {
                    Ok(()) => Ok(()),
                    Err(rename_err) => {
                        #[cfg(unix)]
                        let is_cross_device_retry = rename_err.raw_os_error() == Some(18);
                        #[cfg(not(unix))]
                        let is_cross_device_retry = false;

                        if is_cross_device_retry {
                            copy(from, to)?;
                            remove_file(from)?;
                            Ok(())
                        } else {
                            Err(rename_err)
                        }
                    }
                }
            } else {
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    const MIN_MP3_BYTES: u64 = 1024;

    macro_rules! test_import_audio {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let source_path = std::path::Path::new($path);
                    let temp = TempDir::new().unwrap();
                    let tmp_path = temp.path().join("tmp.mp3");
                    let target_path = temp.path().join("target.mp3");

                    let result = import_audio(source_path, &tmp_path, &target_path);
                    assert!(result.is_ok(), "import_audio failed: {:?}", result.err());
                    assert!(target_path.exists());

                    let size = std::fs::metadata(&target_path).unwrap().len();
                    assert!(
                        size > MIN_MP3_BYTES,
                        "Output too small ({size} bytes), likely empty audio"
                    );
                }
            )*
        };
    }

    test_import_audio! {
        test_import_wav: hypr_data::english_1::AUDIO_PATH,
        test_import_mp3: hypr_data::english_1::AUDIO_MP3_PATH,
        test_import_mp4: hypr_data::english_1::AUDIO_MP4_PATH,
        test_import_m4a: hypr_data::english_1::AUDIO_M4A_PATH,
        test_import_ogg: hypr_data::english_1::AUDIO_OGG_PATH,
        test_import_flac: hypr_data::english_1::AUDIO_FLAC_PATH,
        test_import_aac: hypr_data::english_1::AUDIO_AAC_PATH,
        test_import_aiff: hypr_data::english_1::AUDIO_AIFF_PATH,
        test_import_caf: hypr_data::english_1::AUDIO_CAF_PATH,
    }

    #[test]
    fn test_decode_to_mp3_problem_m4a() {
        let path = match std::env::var("PROBLEM_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let bytes = decode_to_mp3(&path).unwrap();
        assert!(
            bytes.len() > MIN_MP3_BYTES as usize,
            "Output too small ({} bytes)",
            bytes.len()
        );
    }

    #[test]
    fn test_decode_to_mp3_problem2_m4a() {
        let path = match std::env::var("PROBLEM2_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let bytes = decode_to_mp3(&path).unwrap();
        assert!(
            bytes.len() > MIN_MP3_BYTES as usize,
            "Output too small ({} bytes)",
            bytes.len()
        );
    }

    #[test]
    fn test_encode_source_to_mp3_preserves_duration_for_stereo_resample() {
        let channels = std::num::NonZeroU16::new(2).unwrap();
        let sample_rate = std::num::NonZeroU32::new(44_100).unwrap();
        let source = rodio::buffer::SamplesBuffer::new(
            channels,
            sample_rate,
            vec![0.5f32; 44_100 * 5 * usize::from(channels.get())],
        );

        let bytes = encode_source_to_mp3(source).unwrap();
        assert!(bytes.len() > MIN_MP3_BYTES as usize);

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("encoded.mp3");
        std::fs::write(&path, &bytes).unwrap();

        let decoder = rodio::Decoder::try_from(File::open(&path).unwrap()).unwrap();
        let output_rate: u32 = decoder.sample_rate().into();
        let output_channels: u16 = decoder.channels().into();
        let output_samples: Vec<f32> = decoder.collect();

        assert_eq!(output_rate, TARGET_SAMPLE_RATE_HZ);
        assert_eq!(output_channels, 1);

        let actual_frames = output_samples.len();
        let expected_frames = TARGET_SAMPLE_RATE_HZ as usize * 5;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.03,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})",
        );
    }
}
