mod error;
pub use error::*;

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
    #[serde(rename = "moonshine-onnx-tiny")]
    #[strum(serialize = "moonshine-onnx-tiny")]
    MoonshineOnnxTiny,
    #[serde(rename = "moonshine-onnx-tiny-q4")]
    #[strum(serialize = "moonshine-onnx-tiny-q4")]
    MoonshineOnnxTinyQ4,
    #[serde(rename = "moonshine-onnx-tiny-q8")]
    #[strum(serialize = "moonshine-onnx-tiny-q8")]
    MoonshineOnnxTinyQ8,
    #[serde(rename = "moonshine-onnx-base")]
    #[strum(serialize = "moonshine-onnx-base")]
    MoonshineOnnxBase,
    #[serde(rename = "moonshine-onnx-base-q4")]
    #[strum(serialize = "moonshine-onnx-base-q4")]
    MoonshineOnnxBaseQ4,
    #[serde(rename = "moonshine-onnx-base-q8")]
    #[strum(serialize = "moonshine-onnx-base-q8")]
    MoonshineOnnxBaseQ8,
}

impl TryFrom<Model> for HyprWhisper {
    type Error = crate::Error;

    fn try_from(model: Model) -> Result<Self, Self::Error> {
        match model {
            Model::WhisperCppTinyQ8 => Ok(HyprWhisper::QuantizedTiny),
            Model::WhisperCppTinyQ8En => Ok(HyprWhisper::QuantizedTinyEn),
            Model::WhisperCppBaseQ8 => Ok(HyprWhisper::QuantizedBase),
            Model::WhisperCppBaseQ8En => Ok(HyprWhisper::QuantizedBaseEn),
            Model::WhisperCppSmallQ8 => Ok(HyprWhisper::QuantizedSmall),
            Model::WhisperCppSmallQ8En => Ok(HyprWhisper::QuantizedSmallEn),
            Model::WhisperCppLargeTurboQ8 => Ok(HyprWhisper::QuantizedLargeTurbo),
            Model::MoonshineOnnxTiny => Err(Error::NotSupported),
            Model::MoonshineOnnxTinyQ4 => Err(Error::NotSupported),
            Model::MoonshineOnnxTinyQ8 => Err(Error::NotSupported),
            Model::MoonshineOnnxBase => Err(Error::NotSupported),
            Model::MoonshineOnnxBaseQ4 => Err(Error::NotSupported),
            Model::MoonshineOnnxBaseQ8 => Err(Error::NotSupported),
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

            Model::MoonshineOnnxBase
            | Model::MoonshineOnnxBaseQ8
            | Model::MoonshineOnnxBaseQ4
            | Model::MoonshineOnnxTiny
            | Model::MoonshineOnnxTinyQ4
            | Model::MoonshineOnnxTinyQ8 => {
                vec![]
            }
        }
    }
}
