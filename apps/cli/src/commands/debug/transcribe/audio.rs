use std::sync::Arc;

use clap::{Args, ValueEnum};
use futures_util::StreamExt;
use owhisper_interface::MixedMessage;

use hypr_audio::{CaptureConfig, CaptureFrame};
use hypr_audio_utils::{chunk_size_for_stt, f32_to_i16_bytes};

pub use hypr_audio::AudioProvider;
pub use hypr_audio_actual::ActualAudio;

use crate::error::{CliError, CliResult};

pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Clone, Copy)]
pub enum ChannelKind {
    Mic,
    Speaker,
}

pub enum DisplayMode {
    Single(ChannelKind),
    Dual,
}

#[derive(Clone, ValueEnum)]
pub enum AudioSource {
    Input,
    Output,
    RawDual,
    AecDual,
    #[cfg(feature = "mock-audio")]
    Mock,
}

impl AudioSource {
    pub fn is_dual(&self) -> bool {
        matches!(self, Self::RawDual | Self::AecDual)
    }

    fn uses_aec(&self) -> bool {
        matches!(self, Self::AecDual)
    }

    #[cfg(feature = "mock-audio")]
    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock)
    }
}

#[derive(Args)]
pub struct AudioArgs {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioSource,
}

pub fn create_single_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> CliResult<
    std::pin::Pin<
        Box<
            dyn futures_util::Stream<
                    Item = MixedMessage<bytes::Bytes, owhisper_interface::ControlMessage>,
                > + Send,
        >,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let use_mic = match source {
        AudioSource::Input => true,
        AudioSource::Output => false,
        #[cfg(feature = "mock-audio")]
        AudioSource::Mock => true,
        AudioSource::RawDual | AudioSource::AecDual => {
            return Err(CliError::operation_failed(
                "create single audio stream",
                "dual audio modes use create_dual_audio_stream",
            ));
        }
    };

    if use_mic {
        let capture = audio
            .open_mic_capture(None, sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open mic capture", e.to_string()))?;
        Ok(Box::pin(capture.filter_map(|result| async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(f32_to_i16_bytes(
                    frame.raw_mic.iter().copied(),
                ))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        })))
    } else {
        let capture = audio
            .open_speaker_capture(sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open speaker capture", e.to_string()))?;
        Ok(Box::pin(capture.filter_map(|result| async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(f32_to_i16_bytes(
                    frame.raw_speaker.iter().copied(),
                ))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        })))
    }
}

pub fn create_dual_audio_stream(
    audio: &Arc<dyn AudioProvider>,
    source: &AudioSource,
    sample_rate: u32,
) -> CliResult<
    std::pin::Pin<
        Box<
            dyn futures_util::Stream<
                    Item = MixedMessage<
                        (bytes::Bytes, bytes::Bytes),
                        owhisper_interface::ControlMessage,
                    >,
                > + Send,
        >,
    >,
> {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let capture_stream = audio
        .open_capture(CaptureConfig {
            sample_rate,
            chunk_size,
            mic_device: None,
            enable_aec: source.uses_aec(),
        })
        .map_err(|e| CliError::operation_failed("open realtime capture", e.to_string()))?;
    let source = source.clone();

    Ok(Box::pin(capture_stream.filter_map(move |result| {
        let source = source.clone();
        async move {
            match result {
                Ok(frame) => Some(MixedMessage::Audio(capture_frame_to_bytes(&source, frame))),
                Err(error) => {
                    eprintln!("capture failed: {error}");
                    None
                }
            }
        }
    })))
}

pub fn print_audio_info(audio: &dyn AudioProvider, source: &AudioSource, sample_rate: u32) {
    let source_name = match source {
        AudioSource::Input => "input",
        AudioSource::Output => "output",
        #[cfg(feature = "mock-audio")]
        AudioSource::Mock => "mock",
        AudioSource::RawDual | AudioSource::AecDual => unreachable!(),
    };
    let chunk_size = chunk_size_for_stt(sample_rate);

    eprintln!("source: {} ({})", source_name, audio.default_device_name());
    eprintln!(
        "sample rate: {} Hz, chunk size: {} samples",
        sample_rate, chunk_size
    );
    eprintln!();
}

pub fn print_dual_audio_info(audio: &dyn AudioProvider, source: &AudioSource, sample_rate: u32) {
    let chunk_size = chunk_size_for_stt(sample_rate);
    let source_name = match source {
        AudioSource::RawDual => "raw-dual",
        AudioSource::AecDual => "aec-dual",
        AudioSource::Input | AudioSource::Output => unreachable!(),
        #[cfg(feature = "mock-audio")]
        AudioSource::Mock => unreachable!(),
    };

    eprintln!(
        "source: {} (input: {}, output: RealtimeSpeaker)",
        source_name,
        audio.default_device_name()
    );
    eprintln!(
        "sample rate: {} Hz, chunk size: {} samples, AEC: {}",
        sample_rate,
        chunk_size,
        if source.uses_aec() {
            "enabled"
        } else {
            "disabled"
        }
    );
    eprintln!();
}

pub fn capture_frame_to_bytes(
    source: &AudioSource,
    frame: CaptureFrame,
) -> (bytes::Bytes, bytes::Bytes) {
    let (mic, speaker) = match source {
        AudioSource::RawDual => frame.raw_dual(),
        AudioSource::AecDual => frame.aec_dual(),
        AudioSource::Input | AudioSource::Output => unreachable!(),
        #[cfg(feature = "mock-audio")]
        AudioSource::Mock => unreachable!(),
    };

    (
        f32_to_i16_bytes(mic.iter().copied()),
        f32_to_i16_bytes(speaker.iter().copied()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_source_reports_dual_modes() {
        assert!(!AudioSource::Input.is_dual());
        assert!(!AudioSource::Output.is_dual());
        assert!(AudioSource::RawDual.is_dual());
        assert!(AudioSource::AecDual.is_dual());
    }

    #[test]
    fn capture_frame_to_bytes_preserves_channel_order() {
        let frame = CaptureFrame {
            raw_mic: std::sync::Arc::from([0.25_f32, -0.25]),
            raw_speaker: std::sync::Arc::from([0.75_f32, -0.75]),
            aec_mic: Some(std::sync::Arc::from([0.1_f32, -0.1])),
        };

        let (raw_mic, raw_speaker) = capture_frame_to_bytes(&AudioSource::RawDual, frame.clone());
        assert_eq!(&raw_mic[..], &[0x00, 0x20, 0x00, 0xe0]);
        assert_eq!(&raw_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);

        let (aec_mic, aec_speaker) = capture_frame_to_bytes(&AudioSource::AecDual, frame);
        assert_eq!(&aec_mic[..], &[0xcc, 0x0c, 0x34, 0xf3]);
        assert_eq!(&aec_speaker[..], &[0x00, 0x60, 0x00, 0xa0]);
    }
}
