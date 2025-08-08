use hypr_whisper_local_model::WhisperModel as HyprWhisper;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display, clap::ValueEnum)]
pub enum Model {
    #[serde(rename = "whisper-cpp-base-q8")]
    #[strum(serialize = "whisper-cpp-base-q8")]
    WhisperCppBaseQ8,
    #[serde(rename = "whisper-cpp-base-q8-en")]
    #[strum(serialize = "whisper-cpp-base-q8-en")]
    WhisperCppBaseQ8En,
    #[serde(rename = "whisper-cpp-tiny-q8")]
    #[strum(serialize = "whisper-cpp-tiny-q8")]
    WhisperCppTinyQ8,
    #[serde(rename = "whisper-cpp-tiny-q8-en")]
    #[strum(serialize = "whisper-cpp-tiny-q8-en")]
    WhisperCppTinyQ8En,
    #[serde(rename = "whisper-cpp-small-q8")]
    #[strum(serialize = "whisper-cpp-small-q8")]
    WhisperCppSmallQ8,
    #[serde(rename = "whisper-cpp-small-q8-en")]
    #[strum(serialize = "whisper-cpp-small-q8-en")]
    WhisperCppSmallQ8En,
    #[serde(rename = "whisper-cpp-large-turbo-q8")]
    #[strum(serialize = "whisper-cpp-large-turbo-q8")]
    WhisperCppLargeTurboQ8,
    #[serde(rename = "kyutai-stt-1b-en-fr")]
    #[strum(serialize = "kyutai-stt-1b-en-fr")]
    KyutaiStt1bEnFr,
}

impl TryFrom<Model> for HyprWhisper {
    type Error = String;

    fn try_from(model: Model) -> Result<Self, Self::Error> {
        match model {
            Model::WhisperCppTinyQ8 => Ok(HyprWhisper::QuantizedTiny),
            Model::WhisperCppTinyQ8En => Ok(HyprWhisper::QuantizedTinyEn),
            Model::WhisperCppBaseQ8 => Ok(HyprWhisper::QuantizedBase),
            Model::WhisperCppBaseQ8En => Ok(HyprWhisper::QuantizedBaseEn),
            Model::WhisperCppSmallQ8 => Ok(HyprWhisper::QuantizedSmall),
            Model::WhisperCppSmallQ8En => Ok(HyprWhisper::QuantizedSmallEn),
            Model::WhisperCppLargeTurboQ8 => Ok(HyprWhisper::QuantizedLargeTurbo),
            Model::KyutaiStt1bEnFr => Err("not_supported".to_string()),
        }
    }
}

#[derive(Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub checksum: String,
}

impl Model {
    pub fn assets(&self) -> Vec<Asset> {
        match self {
            Model::WhisperCppTinyQ8
            | Model::WhisperCppTinyQ8En
            | Model::WhisperCppBaseQ8
            | Model::WhisperCppBaseQ8En
            | Model::WhisperCppSmallQ8
            | Model::WhisperCppSmallQ8En
            | Model::WhisperCppLargeTurboQ8 => {
                let hypr_model: HyprWhisper = self.clone().try_into().unwrap();

                vec![Asset {
                    name: hypr_model.file_name().to_string(),
                    url: hypr_model.model_url().to_string(),
                    size: hypr_model.model_size(),
                    checksum: hypr_model.checksum().to_string(),
                }]
            }
            Model::KyutaiStt1bEnFr => {
                vec![]
            }
        }
    }
}
