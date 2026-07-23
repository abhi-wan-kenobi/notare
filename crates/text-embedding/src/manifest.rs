//! Pinned artifact manifest for the EmbeddingGemma-300M ONNX model
//! (download-on-first-run; the app never bundles the weights — Gemma-terms
//! decision, 2026-07-22 scope call #4).
//!
//! SHA-256 digests are the integrity source of truth for this crate's
//! [`verify_artifacts`]. CRC32 + sizes are ALSO recorded so the consuming
//! plugin (WS-B1 PR9) can hand these straight to `model-downloader`'s
//! `DownloadPart` (which verifies CRC32 + size).

use sha2::{Digest, Sha256};

pub const MODEL_DIR_NAME: &str = "text-embedding/embeddinggemma-300m-q8";

pub const MODEL_FILE: &str = "model_quantized.onnx";
pub const MODEL_DATA_FILE: &str = "model_quantized.onnx_data";
pub const TOKENIZER_FILE: &str = "tokenizer.json";

#[derive(Debug, Clone, Copy)]
pub struct Artifact {
    /// File name inside the model directory.
    pub name: &'static str,
    /// Download URL (un-gated `onnx-community` mirror of the Gemma-licensed
    /// model; the download UI must surface the Gemma terms link).
    pub url: &'static str,
    pub sha256: &'static str,
    pub crc32: u32,
    pub size: u64,
}

/// All files required by [`crate::TextEmbedder::load`], pinned to the
/// `onnx-community/embeddinggemma-300m-ONNX` revision verified on 2026-07-23.
pub const ARTIFACTS: &[Artifact] = &[
    Artifact {
        name: MODEL_FILE,
        url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_quantized.onnx",
        sha256: "172efde319fe1542dc41f31be6154910b05b78f7a861c265c4600eec906bd6d8",
        crc32: 0x72c8_cda8,
        size: 567_874,
    },
    Artifact {
        name: MODEL_DATA_FILE,
        url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_quantized.onnx_data",
        sha256: "705626e28e4c23c82ade34566b4197d97f534c12275fa406dfb71e9937d388c0",
        crc32: 0xac7e_c659,
        size: 308_890_624,
    },
    Artifact {
        name: TOKENIZER_FILE,
        url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/tokenizer.json",
        sha256: "4dda02faaf32bc91031dc8c88457ac272b00c1016cc679757d1c441b248b9c47",
        crc32: 0x9d82_bfad,
        size: 20_323_312,
    },
];

/// Verify every artifact in `model_dir` against the pinned SHA-256 digests.
/// Returns `Ok(())` only when all files exist, have the pinned size, and hash
/// to the pinned digest. Streaming read; ~330 MB total.
pub fn verify_artifacts(model_dir: impl AsRef<std::path::Path>) -> crate::Result<()> {
    let dir = model_dir.as_ref();
    for artifact in ARTIFACTS {
        use std::io::Read;
        let path = dir.join(artifact.name);
        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let actual = hex_lower(&hasher.finalize());
        if actual != artifact.sha256 {
            return Err(crate::Error::IntegrityMismatch {
                name: artifact.name,
                expected: artifact.sha256,
                actual,
            });
        }
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}
