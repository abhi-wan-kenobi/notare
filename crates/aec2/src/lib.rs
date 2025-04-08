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
        if input_frame.len() != self.config.frame_size {
            return Err(Error::InvalidInputFrameLength);
        }

        if reference_frame.len() != self.config.frame_size {
            return Err(Error::InvalidReferenceFrameLength);
        }

        self.inner
            .cancel_echo(input_frame, reference_frame, out_buffer);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;

    #[test]
    fn test_aec() {
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../aec/data");

        // all pcm_s16le, 16k, 1chan.
        let lpb_wav_reader = WavReader::open(data_dir.join("doubletalk_lpb_sample.wav")).unwrap();
        let mic_wav_reader = WavReader::open(data_dir.join("doubletalk_mic_sample.wav")).unwrap();

        let _spec = lpb_wav_reader.spec();
        let lpb_data: Vec<i16> = lpb_wav_reader
            .into_samples()
            .filter_map(Result::ok)
            .collect();
        let _mic_data: Vec<i16> = mic_wav_reader
            .into_samples()
            .filter_map(Result::ok)
            .take(lpb_data.len())
            .collect();

        let _aec = AEC::builder().sample_rate(16000).build();

        // let mut out_buffer = vec![0i16; lpb_data.len()];

        // aec.process(&lpb_data, &mic_data, &mut out_buffer).unwrap();

        // let mut writer =
        //     WavWriter::create(data_dir.join("doubletalk_aec2_output.wav"), spec).unwrap();

        // for sample in out_buffer {
        //     writer.write_sample(sample).unwrap();
        // }
        // writer.finalize().unwrap();
    }
}
