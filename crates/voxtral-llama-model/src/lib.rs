//! Catalog metadata for the Voxtral Mini 3B GGUF model, served through
//! llama.cpp's `libmtmd` audio path (see `transcribe-voxtral-llama`).
//!
//! Mirrors the `parakeet-onnx-model` vs `parakeet-onnx` split (no runtime
//! dependency here) and its multi-file model shape: Voxtral ships as a
//! text-decoder GGUF plus a separate `mmproj` audio-encoder GGUF, the same
//! "directory of fixed-name files" pattern Parakeet already proved out for
//! the download/integrity code.

/// Pinned revision of
/// <https://huggingface.co/ggml-org/Voxtral-Mini-3B-2507-GGUF>. All file
/// URLs, sizes and CRC32s below are for exactly this commit.
const HF_REPO: &str = "ggml-org/Voxtral-Mini-3B-2507-GGUF";
const HF_REVISION: &str = "20616573013f28229fff61de74359d3eeff61f6a";

/// One file of a multi-file model artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    /// File name inside the model directory (also the path suffix on HF).
    pub name: &'static str,
    pub size_bytes: u64,
    pub crc32: u32,
}

/// GGUF text-decoder weight (Q4_K_M quant, ~2.47GB) — verified via a real
/// download + `crc32fast::hash` on 2026-07-17.
const MINI_3B_Q4_K_M_FILES: &[ModelFile] = &[
    ModelFile {
        name: "Voxtral-Mini-3B-2507-Q4_K_M.gguf",
        size_bytes: 2_473_001_920,
        crc32: 581_004_535,
    },
    ModelFile {
        name: "mmproj-Voxtral-Mini-3B-2507-Q8_0.gguf",
        size_bytes: 715_714_080,
        crc32: 3_850_734_077,
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
pub enum VoxtralLlamaModel {
    #[serde(rename = "voxtral-mini-3b-2507-q4km")]
    #[strum(serialize = "voxtral-mini-3b-2507-q4km")]
    Mini3bQ4KM,
}

impl VoxtralLlamaModel {
    pub fn all() -> &'static [VoxtralLlamaModel] {
        &[VoxtralLlamaModel::Mini3bQ4KM]
    }

    /// Directory name (under `models/stt/`) holding the model files.
    pub fn model_dir(&self) -> &'static str {
        match self {
            VoxtralLlamaModel::Mini3bQ4KM => "voxtral-mini-3b-2507-q4km",
        }
    }

    pub fn files(&self) -> &'static [ModelFile] {
        match self {
            VoxtralLlamaModel::Mini3bQ4KM => MINI_3B_Q4_K_M_FILES,
        }
    }

    /// The GGUF text-decoder weight file, out of [`Self::files`].
    pub fn weight_file(&self) -> &'static str {
        match self {
            VoxtralLlamaModel::Mini3bQ4KM => "Voxtral-Mini-3B-2507-Q4_K_M.gguf",
        }
    }

    /// The `mmproj` audio-encoder file, out of [`Self::files`].
    pub fn mmproj_file(&self) -> &'static str {
        match self {
            VoxtralLlamaModel::Mini3bQ4KM => "mmproj-Voxtral-Mini-3B-2507-Q8_0.gguf",
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

    /// Follows the same "<Family> <Variant> (Multilingual/English)" naming
    /// convention every other catalogued STT family uses (see
    /// `WhisperModel::display_name`, `ParakeetOnnxModel::display_name`) —
    /// parameter count/quant lives in `description()`, not here.
    pub fn display_name(&self) -> &'static str {
        match self {
            VoxtralLlamaModel::Mini3bQ4KM => "Voxtral Mini (Multilingual)",
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
        "Voxtral (llama.cpp)"
    }

    /// Voxtral Mini 3B 2507's official "natively multilingual" set (8
    /// languages, incl. Hindi). See `hypr_language::voxtral_mini_3b_languages`.
    pub fn supported_languages(&self) -> Vec<hypr_language::Language> {
        hypr_language::voxtral_mini_3b_languages()
    }

    pub fn supports_languages(&self, languages: &[hypr_language::Language]) -> bool {
        languages
            .iter()
            .all(hypr_language::is_voxtral_mini_3b_language)
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
        let model = VoxtralLlamaModel::Mini3bQ4KM;
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(json, "\"voxtral-mini-3b-2507-q4km\"");
        assert_eq!(model.to_string(), "voxtral-mini-3b-2507-q4km");
        assert_eq!(
            "voxtral-mini-3b-2507-q4km"
                .parse::<VoxtralLlamaModel>()
                .unwrap(),
            model
        );
    }

    #[test]
    fn files_have_pinned_sizes_and_urls() {
        let model = VoxtralLlamaModel::Mini3bQ4KM;
        assert_eq!(model.files().len(), 2);
        assert_eq!(model.model_size_bytes(), 3_188_716_000);

        let url = model.file_url(model.weight_file());
        assert!(url.contains("20616573013f28229fff61de74359d3eeff61f6a"));
        assert!(url.ends_with("/Voxtral-Mini-3B-2507-Q4_K_M.gguf"));

        let mmproj_url = model.file_url(model.mmproj_file());
        assert!(mmproj_url.ends_with("/mmproj-Voxtral-Mini-3B-2507-Q8_0.gguf"));
    }

    #[test]
    fn language_support_is_honest() {
        let model = VoxtralLlamaModel::Mini3bQ4KM;
        assert!(!model.is_english_only());
        assert_eq!(model.supported_languages().len(), 8);

        let en: hypr_language::Language = "en".parse().unwrap();
        let hi: hypr_language::Language = "hi".parse().unwrap();
        let ko: hypr_language::Language = "ko".parse().unwrap();
        assert!(model.supports_languages(&[en, hi]));
        assert!(!model.supports_languages(&[ko]));
    }

    #[test]
    fn description_is_human_readable() {
        assert_eq!(VoxtralLlamaModel::Mini3bQ4KM.description(), "3.0 GB");
    }
}
