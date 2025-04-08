mod error;
pub use error::*;

#[derive(Default)]
pub struct AECBuilder {
    sample_rate: Option<usize>,
}

impl AECBuilder {
    pub fn sample_rate(mut self, sample_rate: usize) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    pub fn build(self) -> AEC {
        let sr = self.sample_rate.unwrap_or(16000);
        let config = aec_rs::AecConfig {
            frame_size: sr / 100,
            filter_length: (sr as i32 / 10),
            sample_rate: sr as u32,
            enable_preprocess: false,
        };
        let inner = aec_rs::Aec::new(&config);

        AEC { config, inner }
    }
}

pub struct AEC {
    config: aec_rs::AecConfig,
    inner: aec_rs::Aec,
}

impl AEC {
    pub fn builder() -> AECBuilder {
        Default::default()
    }

    pub fn process(
        &mut self,
        input_frame: &[i16],
        reference_frame: &[i16],
        out_buffer: &mut [i16],
    ) -> Result<(), Error> {
        let frame_size = self.config.frame_size;

        let max_len = std::cmp::max(
            std::cmp::min(input_frame.len(), out_buffer.len()),
            reference_frame.len(),
        );

        for chunk_idx in 0..(max_len + frame_size - 1) / frame_size {
            let start_idx = chunk_idx * frame_size;

            if start_idx >= out_buffer.len() {
                break;
            }

            let mut chunk_input = vec![0i16; frame_size];
            let mut chunk_reference = vec![0i16; frame_size];
            let mut chunk_output = vec![0i16; frame_size];

            let input_copy_len = if start_idx < input_frame.len() {
                std::cmp::min(input_frame.len() - start_idx, frame_size)
            } else {
                0
            };

            let reference_copy_len = if start_idx < reference_frame.len() {
                std::cmp::min(reference_frame.len() - start_idx, frame_size)
            } else {
                0
            };

            if input_copy_len > 0 {
                chunk_input[..input_copy_len]
                    .copy_from_slice(&input_frame[start_idx..start_idx + input_copy_len]);
            }

            if reference_copy_len > 0 {
                chunk_reference[..reference_copy_len]
                    .copy_from_slice(&reference_frame[start_idx..start_idx + reference_copy_len]);
            }

            self.inner
                .cancel_echo(&chunk_input, &chunk_reference, &mut chunk_output);

            let output_copy_len = std::cmp::min(out_buffer.len() - start_idx, frame_size);

            if output_copy_len > 0 {
                out_buffer[start_idx..start_idx + output_copy_len]
                    .copy_from_slice(&chunk_output[..output_copy_len]);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{WavReader, WavWriter};

    #[test]
    fn test_aec() {
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
        let lpb_wav_reader = WavReader::open(data_dir.join("echo.wav")).unwrap();
        let mic_wav_reader = WavReader::open(data_dir.join("rec.wav")).unwrap();

        let spec = lpb_wav_reader.spec();
        let lpb_data: Vec<i16> = lpb_wav_reader
            .into_samples()
            .filter_map(Result::ok)
            .collect();
        let mic_data: Vec<i16> = mic_wav_reader
            .into_samples()
            .filter_map(Result::ok)
            .collect();

        let mut aec = AEC::builder().sample_rate(16000).build();

        let mut out_buffer = vec![0i16; mic_data.len()];

        aec.process(&mic_data, &lpb_data, &mut out_buffer).unwrap();

        let mut writer = WavWriter::create(data_dir.join("output.wav"), spec).unwrap();

        for sample in out_buffer {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }
}
