use std::path::{Path, PathBuf};

use crate::Error;
use crate::model::{DownloadPart, DownloadableModel};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum ModelIntegrity {
    NotInstalled,
    Verified,
    /// Present on disk but no verifiable single-file artifact (e.g. unpacked
    /// archives) or no checksum metadata in the catalog.
    PresentUnverified,
    Corrupt(String),
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VerifiedStamp {
    size: u64,
    mtime_secs: u64,
    crc32: u32,
}

fn stamp_path(destination: &Path) -> PathBuf {
    let mut name = destination.file_name().unwrap_or_default().to_os_string();
    name.push(".verified");
    destination.with_file_name(name)
}

fn file_meta(path: &Path) -> Result<(u64, u64), Error> {
    let meta = std::fs::metadata(path).map_err(|e| Error::OperationFailed(e.to_string()))?;
    let mtime_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok((meta.len(), mtime_secs))
}

pub(crate) fn remove_stamp(destination: &Path) {
    let _ = std::fs::remove_file(stamp_path(destination));
}

/// Verify a model's on-disk reality against its catalog metadata.
///
/// Never trusts stored state: existence, size, and CRC32 are checked against
/// the filesystem. A successful full verification is cached in a sidecar
/// stamp (`<file>.verified`) keyed on size+mtime+expected checksum so
/// multi-gigabyte models are not re-hashed on every launch; any change to the
/// file invalidates the stamp.
pub fn verify_model<M: DownloadableModel>(
    model: &M,
    models_base: &Path,
) -> Result<ModelIntegrity, Error> {
    if !model.is_downloaded(models_base)? {
        return Ok(ModelIntegrity::NotInstalled);
    }

    // Multi-part models install as a directory of files, each individually
    // verifiable against its catalog size + CRC32.
    if let Some(parts) = model.download_parts() {
        return verify_parts(&model.download_destination(models_base), &parts);
    }

    // Models whose installed form differs from the downloaded artifact
    // (e.g. tar archives unpacked into a directory) can't be file-hash
    // verified against the download checksum.
    if model.remove_destination_after_finalize() {
        return Ok(ModelIntegrity::PresentUnverified);
    }

    let destination = model.download_destination(models_base);
    if destination.is_dir() {
        return Ok(ModelIntegrity::PresentUnverified);
    }
    if !destination.is_file() {
        return Ok(ModelIntegrity::NotInstalled);
    }

    let (size, mtime_secs) = file_meta(&destination)?;

    if let Some(expected) = model.expected_size()
        && size != expected
    {
        return Ok(ModelIntegrity::Corrupt(format!(
            "size mismatch: expected {expected} bytes, found {size}"
        )));
    }

    let Some(expected_crc) = model.download_checksum() else {
        return Ok(ModelIntegrity::PresentUnverified);
    };

    match verify_file_with_stamp(&destination, size, mtime_secs, expected_crc)? {
        None => Ok(ModelIntegrity::Verified),
        Some(reason) => Ok(ModelIntegrity::Corrupt(reason)),
    }
}

/// Verify every file of a multi-part model (existence + size + CRC32, with
/// per-file `.verified` stamps as the re-hash fast path).
fn verify_parts(destination_dir: &Path, parts: &[DownloadPart]) -> Result<ModelIntegrity, Error> {
    let mut all_verified = true;

    for part in parts {
        let path = destination_dir.join(&part.relative_path);
        if !path.is_file() {
            return Ok(ModelIntegrity::NotInstalled);
        }

        let (size, mtime_secs) = file_meta(&path)?;

        if let Some(expected) = part.expected_size
            && size != expected
        {
            return Ok(ModelIntegrity::Corrupt(format!(
                "{}: size mismatch: expected {expected} bytes, found {size}",
                part.relative_path
            )));
        }

        let Some(expected_crc) = part.checksum else {
            all_verified = false;
            continue;
        };

        if let Some(reason) = verify_file_with_stamp(&path, size, mtime_secs, expected_crc)? {
            return Ok(ModelIntegrity::Corrupt(format!(
                "{}: {reason}",
                part.relative_path
            )));
        }
    }

    if all_verified {
        Ok(ModelIntegrity::Verified)
    } else {
        Ok(ModelIntegrity::PresentUnverified)
    }
}

/// CRC-check one file, honoring/refreshing its `.verified` sidecar stamp.
/// Returns `None` when the file verifies, `Some(reason)` when corrupt.
fn verify_file_with_stamp(
    path: &Path,
    size: u64,
    mtime_secs: u64,
    expected_crc: u32,
) -> Result<Option<String>, Error> {
    let stamp = stamp_path(path);
    if let Ok(bytes) = std::fs::read(&stamp)
        && let Ok(s) = serde_json::from_slice::<VerifiedStamp>(&bytes)
        && s.size == size
        && s.mtime_secs == mtime_secs
        && s.crc32 == expected_crc
    {
        return Ok(None);
    }

    let actual = hypr_file::calculate_file_checksum(path)
        .map_err(|e| Error::OperationFailed(e.to_string()))?;
    if actual != expected_crc {
        return Ok(Some(format!(
            "checksum mismatch: expected {expected_crc}, found {actual}"
        )));
    }

    let _ = std::fs::write(
        &stamp,
        serde_json::to_vec(&VerifiedStamp {
            size,
            mtime_secs,
            crc32: actual,
        })
        .unwrap_or_default(),
    );
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct FakeModel {
        file_name: &'static str,
        size: Option<u64>,
        crc32: Option<u32>,
    }

    impl DownloadableModel for FakeModel {
        fn download_key(&self) -> String {
            format!("fake:{}", self.file_name)
        }
        fn download_url(&self) -> Option<String> {
            None
        }
        fn download_checksum(&self) -> Option<u32> {
            self.crc32
        }
        fn expected_size(&self) -> Option<u64> {
            self.size
        }
        fn download_destination(&self, models_base: &Path) -> PathBuf {
            models_base.join(self.file_name)
        }
        fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
            Ok(models_base.join(self.file_name).is_file())
        }
        fn finalize_download(&self, _: &Path, _: &Path) -> Result<(), Error> {
            Ok(())
        }
        fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
            let _ = std::fs::remove_file(models_base.join(self.file_name));
            Ok(())
        }
    }

    const CONTENT: &[u8] = b"model-bytes";

    fn model() -> FakeModel {
        FakeModel {
            file_name: "model.bin",
            size: Some(CONTENT.len() as u64),
            crc32: Some(crc32fast::hash(CONTENT)),
        }
    }

    #[test]
    fn missing_file_is_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let state = verify_model(&model(), dir.path()).unwrap();
        assert_eq!(state, ModelIntegrity::NotInstalled);
    }

    #[test]
    fn intact_file_verifies_and_stamps() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.bin"), CONTENT).unwrap();

        assert_eq!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Verified
        );
        assert!(dir.path().join("model.bin.verified").is_file());
        // Second run takes the stamp fast-path and still verifies.
        assert_eq!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Verified
        );
    }

    #[test]
    fn truncated_file_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.bin"), &CONTENT[..4]).unwrap();

        assert!(matches!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Corrupt(_)
        ));
    }

    #[test]
    fn wrong_content_same_size_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.bin"), b"wrong-bytes").unwrap();

        assert!(matches!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Corrupt(_)
        ));
    }

    #[test]
    fn stale_stamp_does_not_mask_swapped_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.bin");
        std::fs::write(&path, CONTENT).unwrap();
        assert_eq!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Verified
        );

        // Same size, different bytes, mtime pushed forward: the stamp must
        // be invalidated and the checksum re-run.
        std::fs::write(&path, b"wrong-bytes").unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        let _ = filetime_set(&path, future);

        assert!(matches!(
            verify_model(&model(), dir.path()).unwrap(),
            ModelIntegrity::Corrupt(_)
        ));
    }

    fn filetime_set(path: &Path, t: std::time::SystemTime) -> std::io::Result<()> {
        let f = std::fs::OpenOptions::new().append(true).open(path)?;
        f.set_modified(t)
    }

    #[test]
    fn no_checksum_metadata_is_present_unverified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("model.bin"), CONTENT).unwrap();
        let m = FakeModel {
            crc32: None,
            ..model()
        };
        assert_eq!(
            verify_model(&m, dir.path()).unwrap(),
            ModelIntegrity::PresentUnverified
        );
    }
}
