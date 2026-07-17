//! Catalog metadata for the Parakeet TDT ONNX models (no `ort` dependency;
//! mirrors the `whisper-local-model` vs `whisper-local` split).

/// Pinned revision of <https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx>.
/// All file URLs, sizes and CRC32s below are for exactly this commit.
const HF_REPO: &str = "istupakov/parakeet-tdt-0.6b-v3-onnx";
const HF_REVISION: &str = "8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce";

/// One file of a multi-file model artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    /// File name inside the model directory (also the path suffix on HF).
    pub name: &'static str,
    pub size_bytes: u64,
    pub crc32: u32,
}

const TDT_V3_INT8_FILES: &[ModelFile] = &[
    ModelFile {
        name: "encoder-model.int8.onnx",
        size_bytes: 652_183_999,
        crc32: 3_581_566_272,
    },
    ModelFile {
        name: "decoder_joint-model.int8.onnx",
        size_bytes: 18_202_004,
        crc32: 1_976_525_640,
    },
    ModelFile {
        name: "nemo128.onnx",
        size_bytes: 139_764,
        crc32: 2_170_743_478,
    },
    ModelFile {
        name: "vocab.txt",
        size_bytes: 93_939,
        crc32: 4_146_995_366,
    },
];

#[derive(
    Debug,
    Eq,
    Hash,
    PartialEq,
    Clone,
    strum::EnumString,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
)]
pub enum ParakeetOnnxModel {
    #[serde(rename = "parakeet-tdt-v3-int8")]
    #[strum(serialize = "parakeet-tdt-v3-int8")]
    TdtV3Int8,
}

impl ParakeetOnnxModel {
    pub fn all() -> &'static [ParakeetOnnxModel] {
        &[ParakeetOnnxModel::TdtV3Int8]
    }

    /// Directory name (under `models/stt/`) holding the model files.
    pub fn model_dir(&self) -> &'static str {
        match self {
            ParakeetOnnxModel::TdtV3Int8 => "parakeet-tdt-0.6b-v3-int8",
        }
    }

    pub fn files(&self) -> &'static [ModelFile] {
        match self {
            ParakeetOnnxModel::TdtV3Int8 => TDT_V3_INT8_FILES,
        }
    }

    /// Download URL for one of [`Self::files`], pinned to a fixed revision so
    /// the hardcoded sizes/CRCs can never drift out from under us.
    pub fn file_url(&self, name: &str) -> String {
        format!("https://huggingface.co/{HF_REPO}/resolve/{HF_REVISION}/{name}")
    }

    pub fn model_size_bytes(&self) -> u64 {
        self.files().iter().map(|file| file.size_bytes).sum()
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ParakeetOnnxModel::TdtV3Int8 => "Parakeet TDT v3 (Multilingual)",
        }
    }

    pub fn description(&self) -> String {
        let mb = self.model_size_bytes() / (1024 * 1024);
        if mb >= 1024 {
            format!("{:.1} GB", mb as f64 / 1024.0)
        } else {
            format!("{} MB", mb)
        }
    }

    /// Runtime that executes this model.
    pub fn engine(&self) -> &'static str {
        "Parakeet (ONNX)"
    }

    /// Parakeet TDT v3 covers 25 (mostly European) languages. No Hindi.
    pub fn supported_languages(&self) -> Vec<hypr_language::Language> {
        hypr_language::parakeet_tdt_v3_languages()
    }

    pub fn supports_languages(&self, languages: &[hypr_language::Language]) -> bool {
        languages
            .iter()
            .all(hypr_language::is_parakeet_tdt_v3_language)
    }

    pub fn is_english_only(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_and_strum_agree_on_the_wire_name() {
        let model = ParakeetOnnxModel::TdtV3Int8;
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(json, "\"parakeet-tdt-v3-int8\"");
        assert_eq!(model.to_string(), "parakeet-tdt-v3-int8");
        assert_eq!(
            "parakeet-tdt-v3-int8".parse::<ParakeetOnnxModel>().unwrap(),
            model
        );
    }

    #[test]
    fn files_have_pinned_sizes_and_urls() {
        let model = ParakeetOnnxModel::TdtV3Int8;
        assert_eq!(model.files().len(), 4);
        assert_eq!(model.model_size_bytes(), 670_619_706);

        let url = model.file_url("vocab.txt");
        assert!(url.contains("8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce"));
        assert!(url.ends_with("/vocab.txt"));
    }

    #[test]
    fn language_support_is_honest() {
        let model = ParakeetOnnxModel::TdtV3Int8;
        assert!(!model.is_english_only());
        assert_eq!(model.supported_languages().len(), 25);

        let en: hypr_language::Language = "en".parse().unwrap();
        let fr: hypr_language::Language = "fr".parse().unwrap();
        let hi: hypr_language::Language = "hi".parse().unwrap();
        assert!(model.supports_languages(&[en, fr]));
        assert!(!model.supports_languages(&[hi]));
    }

    #[test]
    fn description_is_human_readable() {
        assert_eq!(ParakeetOnnxModel::TdtV3Int8.description(), "639 MB");
    }
}
