use std::ffi::CString;
use std::path::Path;

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, parse_buf};
use crate::model::Model;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DiarizeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_powerset: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiarizeResult {
    pub num_speakers: usize,
    pub scores: Vec<f32>,
    pub total_time_ms: f64,
    #[serde(default)]
    pub ram_usage_mb: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SpeakerEmbeddingOptions {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpeakerEmbeddingResult {
    pub embedding: Vec<f32>,
    pub total_time_ms: f64,
    #[serde(default)]
    pub ram_usage_mb: f64,
}

#[derive(serde::Deserialize)]
struct RawDiarizeResponse {
    success: bool,
    error: Option<String>,
    #[serde(default)]
    num_speakers: usize,
    #[serde(default)]
    scores: Vec<f32>,
    #[serde(default)]
    total_time_ms: f64,
    #[serde(default)]
    ram_usage_mb: f64,
}

#[derive(serde::Deserialize)]
struct RawSpeakerEmbeddingResponse {
    success: bool,
    error: Option<String>,
    #[serde(default)]
    embedding: Vec<f32>,
    #[serde(default)]
    total_time_ms: f64,
    #[serde(default)]
    ram_usage_mb: f64,
}

impl Model {
    fn call_diarize(
        &self,
        path: Option<&CString>,
        pcm: Option<&[u8]>,
        options: &DiarizeOptions,
    ) -> Result<DiarizeResult> {
        let guard = self.lock_inference();
        let options_c = CString::new(serde_json::to_string(options)?)?;
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let (pcm_ptr, pcm_len) = pcm
            .map(|p| (p.as_ptr(), p.len()))
            .unwrap_or((std::ptr::null(), 0));

        let rc = unsafe {
            cactus_sys::cactus_diarize(
                guard.raw_handle(),
                path.map_or(std::ptr::null(), |p| p.as_ptr()),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
                options_c.as_ptr(),
                pcm_ptr,
                pcm_len,
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!("cactus_diarize failed ({rc})")));
        }

        let resp: RawDiarizeResponse = parse_buf(&buf)
            .map_err(|e| Error::Inference(format!("failed to parse diarize response: {e}")))?;

        if !resp.success {
            return Err(Error::Inference(
                resp.error.unwrap_or_else(|| "unknown diarize error".into()),
            ));
        }

        Ok(DiarizeResult {
            num_speakers: resp.num_speakers,
            scores: resp.scores,
            total_time_ms: resp.total_time_ms,
            ram_usage_mb: resp.ram_usage_mb,
        })
    }

    pub fn diarize_file(
        &self,
        audio_path: impl AsRef<Path>,
        options: &DiarizeOptions,
    ) -> Result<DiarizeResult> {
        let path_c = CString::new(audio_path.as_ref().to_string_lossy().into_owned())?;
        self.call_diarize(Some(&path_c), None, options)
    }

    pub fn diarize_pcm(&self, pcm: &[u8], options: &DiarizeOptions) -> Result<DiarizeResult> {
        self.call_diarize(None, Some(pcm), options)
    }

    fn call_embed_speaker(
        &self,
        path: Option<&CString>,
        pcm: Option<&[u8]>,
        options: &SpeakerEmbeddingOptions,
        mask_weights: Option<&[f32]>,
    ) -> Result<SpeakerEmbeddingResult> {
        let guard = self.lock_inference();
        let options_c = CString::new(serde_json::to_string(options)?)?;
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let (pcm_ptr, pcm_len) = pcm
            .map(|p| (p.as_ptr(), p.len()))
            .unwrap_or((std::ptr::null(), 0));
        let (mask_ptr, mask_len) = mask_weights
            .map(|weights| (weights.as_ptr(), weights.len()))
            .unwrap_or((std::ptr::null(), 0));

        let rc = unsafe {
            cactus_sys::cactus_embed_speaker(
                guard.raw_handle(),
                path.map_or(std::ptr::null(), |p| p.as_ptr()),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
                options_c.as_ptr(),
                pcm_ptr,
                pcm_len,
                mask_ptr,
                mask_len,
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!(
                "cactus_embed_speaker failed ({rc})"
            )));
        }

        let resp: RawSpeakerEmbeddingResponse = parse_buf(&buf).map_err(|e| {
            Error::Inference(format!("failed to parse speaker embedding response: {e}"))
        })?;

        if !resp.success {
            return Err(Error::Inference(
                resp.error
                    .unwrap_or_else(|| "unknown speaker embedding error".into()),
            ));
        }

        Ok(SpeakerEmbeddingResult {
            embedding: resp.embedding,
            total_time_ms: resp.total_time_ms,
            ram_usage_mb: resp.ram_usage_mb,
        })
    }

    pub fn embed_speaker_file(
        &self,
        audio_path: impl AsRef<Path>,
        options: &SpeakerEmbeddingOptions,
        mask_weights: Option<&[f32]>,
    ) -> Result<SpeakerEmbeddingResult> {
        let path_c = CString::new(audio_path.as_ref().to_string_lossy().into_owned())?;
        self.call_embed_speaker(Some(&path_c), None, options, mask_weights)
    }

    pub fn embed_speaker_pcm(
        &self,
        pcm: &[u8],
        options: &SpeakerEmbeddingOptions,
        mask_weights: Option<&[f32]>,
    ) -> Result<SpeakerEmbeddingResult> {
        self.call_embed_speaker(None, Some(pcm), options, mask_weights)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiarizeOptions, SpeakerEmbeddingOptions};

    #[test]
    fn diarize_options_omit_unset_fields() {
        let json = serde_json::to_value(DiarizeOptions::default()).expect("serialize options");
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn speaker_embedding_options_serialize_to_empty_object() {
        let json =
            serde_json::to_value(SpeakerEmbeddingOptions::default()).expect("serialize options");
        assert_eq!(json, serde_json::json!({}));
    }
}
