use std::io::Write;
use std::path::PathBuf;
use std::pin::pin;

use tokio::signal;
use tokio_stream::StreamExt;

use hypr_audio::{AudioProvider, CaptureConfig};
use hypr_audio_actual::ActualAudio;
use hypr_audio_utils::chunk_size_for_stt;

use crate::error::{CliError, CliResult};

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum AudioMode {
    Input,
    Output,
    Dual,
}

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioMode,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "16000")]
    pub sample_rate: u32,
}

pub async fn run(args: Args) -> CliResult<()> {
    let audio = ActualAudio;
    let sample_rate = args.sample_rate;
    let chunk_size = chunk_size_for_stt(sample_rate);

    let output_path = args.output.unwrap_or_else(|| {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let suffix = match args.audio {
            AudioMode::Input => "input",
            AudioMode::Output => "output",
            AudioMode::Dual => "dual",
        };
        PathBuf::from(format!("recording_{ts}_{suffix}.wav"))
    });

    let channels: u16 = match args.audio {
        AudioMode::Input | AudioMode::Output => 1,
        AudioMode::Dual => 2,
    };

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let started = std::time::Instant::now();

    use colored::Colorize;
    eprintln!(
        "{} {} (Ctrl+C to stop) -> {}",
        "recording".green().bold(),
        match args.audio {
            AudioMode::Input => "mic",
            AudioMode::Output => "system audio",
            AudioMode::Dual => "mic + system audio",
        },
        output_path.display()
    );

    let samples = match args.audio {
        AudioMode::Input => record_single(&audio, true, sample_rate, chunk_size).await?,
        AudioMode::Output => record_single(&audio, false, sample_rate, chunk_size).await?,
        AudioMode::Dual => record_dual(&audio, sample_rate, chunk_size).await?,
    };

    let mut writer = hound::WavWriter::create(&output_path, spec)
        .map_err(|e| CliError::operation_failed("create wav file", e.to_string()))?;
    for &sample in &samples {
        writer
            .write_sample(sample)
            .map_err(|e| CliError::operation_failed("write wav sample", e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| CliError::operation_failed("finalize wav", e.to_string()))?;

    let elapsed = started.elapsed();
    let sample_count = samples.len() as u64 / channels as u64;
    let audio_secs = sample_count as f64 / sample_rate as f64;

    eprintln!(
        "{}",
        format!(
            "{:.1}s audio, {:.1}s elapsed -> {}",
            audio_secs,
            elapsed.as_secs_f64(),
            output_path.display()
        )
        .dimmed()
    );

    Ok(())
}

fn to_i16(s: f32) -> i16 {
    (s * 32767.0) as i16
}

async fn record_single(
    audio: &ActualAudio,
    mic: bool,
    sample_rate: u32,
    chunk_size: usize,
) -> CliResult<Vec<i16>> {
    let stream = if mic {
        audio
            .open_mic_capture(None, sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open mic capture", e.to_string()))?
    } else {
        audio
            .open_speaker_capture(sample_rate, chunk_size)
            .map_err(|e| CliError::operation_failed("open speaker capture", e.to_string()))?
    };
    let mut stream = pin!(stream);

    let mut samples: Vec<i16> = Vec::new();
    let mut last_print = std::time::Instant::now();

    loop {
        tokio::select! {
            frame = stream.next() => {
                let Some(result) = frame else { break };
                let frame = result
                    .map_err(|e| CliError::operation_failed("audio capture", e.to_string()))?;
                let raw = if mic { &frame.raw_mic } else { &frame.raw_speaker };
                samples.extend(raw.iter().map(|&s| to_i16(s)));

                if last_print.elapsed().as_secs() >= 1 {
                    let secs = samples.len() as f64 / sample_rate as f64;
                    eprint!("\r  {:.0}s", secs);
                    std::io::stderr().flush().ok();
                    last_print = std::time::Instant::now();
                }
            }
            _ = signal::ctrl_c() => {
                eprintln!();
                break;
            }
        }
    }

    Ok(samples)
}

async fn record_dual(
    audio: &ActualAudio,
    sample_rate: u32,
    chunk_size: usize,
) -> CliResult<Vec<i16>> {
    let stream = audio
        .open_capture(CaptureConfig {
            sample_rate,
            chunk_size,
            mic_device: None,
            enable_aec: false,
        })
        .map_err(|e| CliError::operation_failed("open dual capture", e.to_string()))?;
    let mut stream = pin!(stream);

    let mut samples: Vec<i16> = Vec::new();
    let mut last_print = std::time::Instant::now();

    loop {
        tokio::select! {
            frame = stream.next() => {
                let Some(result) = frame else { break };
                let frame = result
                    .map_err(|e| CliError::operation_failed("audio capture", e.to_string()))?;
                let (mic, speaker) = frame.raw_dual();
                for (&m, &s) in mic.iter().zip(speaker.iter()) {
                    samples.push(to_i16(m));
                    samples.push(to_i16(s));
                }

                if last_print.elapsed().as_secs() >= 1 {
                    let secs = samples.len() as f64 / (sample_rate as f64 * 2.0);
                    eprint!("\r  {:.0}s", secs);
                    std::io::stderr().flush().ok();
                    last_print = std::time::Instant::now();
                }
            }
            _ = signal::ctrl_c() => {
                eprintln!();
                break;
            }
        }
    }

    Ok(samples)
}
