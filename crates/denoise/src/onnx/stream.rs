use super::denoiser::Denoiser;
use super::error::Error;
use super::model::BLOCK_SHIFT;

/// Streaming wrapper around [`Denoiser`] that accepts arbitrary chunk sizes.
///
/// [`Denoiser::process_streaming`] only consumes whole `BLOCK_SHIFT`-sample
/// blocks and zero-fills whatever it could not consume, which makes it
/// unusable on arbitrarily sized capture chunks. This wrapper buffers input
/// to `BLOCK_SHIFT` multiples and returns exactly the denoised samples that
/// were produced (`whole_blocks_consumed * BLOCK_SHIFT`), carrying the
/// remainder over to the next call — no zero tails, no dropped samples.
///
/// Total output always trails total input by less than `BLOCK_SHIFT`
/// (128 samples = 8 ms at 16 kHz), on top of the model's inherent
/// `BLOCK_SIZE - BLOCK_SHIFT` look-behind latency.
pub struct StreamDenoiser {
    denoiser: Denoiser,
    carry: Vec<f32>,
}

impl StreamDenoiser {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            denoiser: Denoiser::new()?,
            carry: Vec::new(),
        })
    }

    /// Clears model state and any carried samples.
    pub fn reset(&mut self) {
        self.denoiser.reset();
        self.carry.clear();
    }

    /// Denoises `input`, returning exactly the samples produced by whole
    /// consumed blocks. The output length is
    /// `((carried + input.len()) / BLOCK_SHIFT) * BLOCK_SHIFT`; the remainder
    /// is buffered for the next call.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, Error> {
        self.carry.extend_from_slice(input);

        let consumable = (self.carry.len() / BLOCK_SHIFT) * BLOCK_SHIFT;
        if consumable == 0 {
            return Ok(Vec::new());
        }

        let output = self.denoiser.process_streaming(&self.carry[..consumable])?;
        debug_assert_eq!(output.len(), consumable);

        self.carry.drain(..consumable);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx::model::BLOCK_SIZE;

    /// Deterministic noisy speech-band test signal, amplitude well below 1.0
    /// so the denoiser's peak-limiting stage never engages (keeping chunked
    /// and one-shot runs bit-comparable).
    fn test_signal(len: usize) -> Vec<f32> {
        let mut state = 0x1234_5678_u32;
        (0..len)
            .map(|i| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (state >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
                let t = i as f32 / 16_000.0;
                let tone = (2.0 * std::f32::consts::PI * 220.0 * t).sin();
                0.25 * tone + 0.2 * noise
            })
            .collect()
    }

    fn chunked_output(signal: &[f32], chunk_size: usize) -> Vec<f32> {
        let mut denoiser = StreamDenoiser::new().unwrap();
        let mut output = Vec::new();
        let mut fed = 0usize;

        for chunk in signal.chunks(chunk_size) {
            let out = denoiser.process(chunk).unwrap();
            output.extend(out);
            fed += chunk.len();

            // Cumulative output is exactly the whole blocks consumed so far.
            assert_eq!(
                output.len(),
                (fed / BLOCK_SHIFT) * BLOCK_SHIFT,
                "cumulative output length wrong at fed={fed}, chunk_size={chunk_size}"
            );
        }

        output
    }

    #[test]
    fn chunked_processing_matches_one_shot_for_all_chunk_sizes() {
        // Deliberately NOT a multiple of BLOCK_SHIFT so a remainder is carried.
        let signal = test_signal(BLOCK_SHIFT * 50 + 37);
        let expected_len = (signal.len() / BLOCK_SHIFT) * BLOCK_SHIFT;

        let one_shot = {
            let mut denoiser = StreamDenoiser::new().unwrap();
            denoiser.process(&signal).unwrap()
        };
        assert_eq!(one_shot.len(), expected_len);

        for &chunk_size in &[100usize, 512, 1000] {
            let chunked = chunked_output(&signal, chunk_size);

            assert_eq!(
                chunked.len(),
                expected_len,
                "total output length wrong for chunk_size={chunk_size}"
            );

            for (i, (a, b)) in chunked.iter().zip(one_shot.iter()).enumerate() {
                assert!(
                    (a - b).abs() < 1e-4,
                    "chunk_size={chunk_size}: sample {i} diverged ({a} vs {b})"
                );
            }
        }
    }

    #[test]
    fn output_has_no_zero_tail() {
        let signal = test_signal(BLOCK_SHIFT * 40);

        let mut denoiser = StreamDenoiser::new().unwrap();
        let output = denoiser.process(&signal).unwrap();
        assert_eq!(output.len(), signal.len());

        // Skip the model's warm-up (first BLOCK_SIZE samples are attenuated by
        // the overlap-add ramp-in); after that the tail must carry signal.
        let tail = &output[output.len() - BLOCK_SHIFT..];
        assert!(
            tail.iter().any(|&x| x.abs() > 1e-6),
            "trailing block is all zeros — zero tail leaked into output"
        );
        assert!(output.len() > BLOCK_SIZE);
        assert!(output.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn small_inputs_buffer_until_a_whole_block_is_available() {
        let signal = test_signal(BLOCK_SHIFT);

        let mut denoiser = StreamDenoiser::new().unwrap();

        let out = denoiser.process(&signal[..BLOCK_SHIFT - 1]).unwrap();
        assert!(out.is_empty(), "sub-block input must produce no output");

        let out = denoiser.process(&signal[BLOCK_SHIFT - 1..]).unwrap();
        assert_eq!(out.len(), BLOCK_SHIFT);

        let out = denoiser.process(&[]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn reset_clears_carry() {
        let mut denoiser = StreamDenoiser::new().unwrap();

        let out = denoiser.process(&test_signal(100)).unwrap();
        assert!(out.is_empty());

        denoiser.reset();

        // If the 100 carried samples had survived reset, feeding 28 more
        // would complete a block and produce output.
        let out = denoiser.process(&test_signal(28)).unwrap();
        assert!(out.is_empty(), "reset must drop carried samples");
    }
}
