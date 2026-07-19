use std::time::{Duration, Instant};

/// Length of the probe audio. Deliberately several seconds, not ~1s: a large
/// model (e.g. Whisper large-v3-turbo) has enough fixed per-request overhead
/// (HTTP, VAD, graph setup) that a 1s clip's realtime factor lands below the
/// GPU threshold even when the GPU is doing the work — the encoder throughput
/// that GPU offload actually accelerates only dominates over a longer clip, so
/// this is what makes "verified" vs "cpu" separate cleanly across model sizes.
const PROBE_AUDIO_SECS: usize = 8;
const PROBE_SAMPLE_RATE: usize = 16000;

/// Generates a `PROBE_AUDIO_SECS`-second 16kHz mono silence WAV file in memory.
fn create_silence_wav() -> Vec<u8> {
    let n_samples = PROBE_AUDIO_SECS * PROBE_SAMPLE_RATE;
    let subchunk2_size: u32 = (n_samples * 2) as u32; // 2 bytes/sample (16-bit PCM)
    let chunk_size: u32 = 36 + subchunk2_size;

    let mut wav = Vec::with_capacity(44 + n_samples * 2);
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&chunk_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM format (1)
    header[22..24].copy_from_slice(&1u16.to_le_bytes()); // Mono channel (1)
    header[24..28].copy_from_slice(&16000u32.to_le_bytes()); // Sample rate (16000)
    header[28..32].copy_from_slice(&32000u32.to_le_bytes()); // Byte rate (16000 * 1ch * 2B/sample; independent of clip length)
    header[32..34].copy_from_slice(&2u16.to_le_bytes()); // Block align (1 channel * 2 bytes/sample)
    header[34..36].copy_from_slice(&16u16.to_le_bytes()); // Bits per sample (16)
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&subchunk2_size.to_le_bytes());

    wav.extend_from_slice(&header);
    wav.extend(std::iter::repeat(0).take(n_samples * 2));
    wav
}

/// Runs a short transcription probe via an HTTP self-request to verify GPU offload and measure performance.
///
/// It sends a multi-second silent WAV segment (PROBE_AUDIO_SECS) to the local `/v1/listen` endpoint.
/// If the request succeeds, it returns the calculated realtime factor (audio duration / elapsed time).
/// If the server is starting up and not yet accepting connections, it retries briefly.
/// Returns `None` if the request fails or if the server returns an error.
/// `token` must match the server's configured shared secret when one is set
/// (`NOTARE_STT_TOKEN`); the probe hits the same auth-gated `/v1/listen` route,
/// so without it the request 401s and the offload factor can never be measured.
pub async fn run_probe(port: u16, token: Option<&str>) -> Option<f32> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/v1/listen?channels=1&sample_rate=16000", port);
    let wav_data = create_silence_wav();

    let mut attempts = 0;
    let max_attempts = 30;
    let retry_delay = Duration::from_millis(100);

    loop {
        attempts += 1;
        let start = Instant::now();
        let mut request = client
            .post(&url)
            .header("content-type", "audio/wav")
            .body(wav_data.clone());
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        match request.send().await {
            Ok(resp) => {
                let elapsed = start.elapsed().as_secs_f32();
                if resp.status().is_success() {
                    let elapsed = if elapsed <= 0.0 { 0.0001 } else { elapsed };
                    let factor = PROBE_AUDIO_SECS as f32 / elapsed;
                    tracing::info!(
                        elapsed_secs = elapsed,
                        realtime_factor = factor,
                        status = %resp.status(),
                        "probe: completed successfully"
                    );
                    return Some(factor);
                } else {
                    tracing::warn!(
                        status = %resp.status(),
                        "probe: request succeeded but server returned error status"
                    );
                    return None;
                }
            }
            Err(error) => {
                if attempts >= max_attempts {
                    tracing::warn!(
                        url = %url,
                        attempts = attempts,
                        %error,
                        "probe: connection failed after max attempts"
                    );
                    return None;
                }
                tracing::debug!(
                    url = %url,
                    attempt = attempts,
                    %error,
                    "probe: connection failed, retrying..."
                );
                tokio::time::sleep(retry_delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    #[test]
    fn test_create_silence_wav_valid() {
        let wav_data = create_silence_wav();
        assert_eq!(wav_data.len(), 44 + PROBE_AUDIO_SECS * PROBE_SAMPLE_RATE * 2);
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");
        assert_eq!(&wav_data[12..16], b"fmt ");
        assert_eq!(&wav_data[36..40], b"data");

        // Verify with rodio that it parses as a valid WAV
        let cursor = std::io::Cursor::new(wav_data);
        let decoder = rodio::Decoder::try_from(cursor);
        assert!(decoder.is_ok());
        let decoder = decoder.unwrap();
        assert_eq!(decoder.channels(), std::num::NonZero::new(1).unwrap());
        assert_eq!(decoder.sample_rate(), std::num::NonZero::new(16000).unwrap());
    }

    #[tokio::test]
    async fn test_run_probe_closed_port_returns_none() {
        // Port 0 is an invalid target port, causing an immediate or eventual connection failure.
        // Verify it retries and returns None without panicking.
        let result = run_probe(0, None).await;
        assert!(result.is_none());
    }
}
