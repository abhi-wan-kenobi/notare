use std::path::Path;
use std::path::PathBuf;

use crate::Error;

/// One file of a multi-file model download.
#[derive(Debug, Clone)]
pub struct DownloadPart {
    pub url: String,
    /// Path of this file relative to the model's `download_destination`
    /// (which is a directory for multi-part models).
    pub relative_path: String,
    /// CRC32 of the file, when known.
    pub checksum: Option<u32>,
    /// Expected byte size, when known. Also used to weight aggregate
    /// download progress.
    pub expected_size: Option<u64>,
}

pub trait DownloadableModel: Clone + Send + Sync + 'static {
    fn download_key(&self) -> String;
    fn download_url(&self) -> Option<String>;
    fn download_checksum(&self) -> Option<u32> {
        None
    }
    /// Expected byte size of the downloaded artifact, when known. Used by
    /// integrity verification to catch truncated/corrupt files cheaply.
    fn expected_size(&self) -> Option<u64> {
        None
    }
    /// Multi-file models return their parts here; `download_destination`
    /// must then point at the directory the parts are installed into.
    /// Single-file models (the default) return `None` and use
    /// `download_url` instead.
    fn download_parts(&self) -> Option<Vec<DownloadPart>> {
        None
    }
    fn download_destination(&self, models_base: &Path) -> PathBuf;
    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error>;
    fn finalize_download(&self, downloaded_path: &Path, models_base: &Path) -> Result<(), Error>;
    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error>;

    fn remove_destination_after_finalize(&self) -> bool {
        false
    }
}
