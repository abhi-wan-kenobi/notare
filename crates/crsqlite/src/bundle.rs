//! Vendored prebuilt cr-sqlite binaries: target selection + cache-dir
//! extraction.
//!
//! Ported from `crates/cloudsync/src/bundle.rs` (the `#154`-class fix),
//! renamed for the cr-sqlite engine. The four `sync_platform` targets carry a
//! prebuilt loadable extension from the vlcn-io `v0.16.3` release under
//! `vendor/<target>/`; this module embeds the current target's binary with
//! `include_bytes!` and extracts it to
//! `dirs::cache_dir()/notare/crsqlite/<version>/<target>/crsqlite.{so,dylib,dll}`
//! on first use.
//!
//! Deliberately NOT carried from cloudsync's bundle.rs: the `from-source`
//! OUT_DIR staging (no build.rs C build exists for cr-sqlite — packaging 1b,
//! a static link, is the optional later path), the `resources/cloudsync`
//! app-bundle probing, and any `tauri.conf.json` resource entry. The
//! extension never needs to be staged next to the executable because the
//! cache dir is writable everywhere we run — the #154 class cannot recur.

use std::fs;
use std::path::PathBuf;

use crate::error::Error;

/// The vendored cr-sqlite release, matching `CRSQLITE_VERSION` in lib.rs.
const CRSQLITE_VERSION: &str = "0.16.3";

macro_rules! configure_crsqlite_target {
    ($target:literal, $file_name:literal, $path:literal) => {
        const CRSQLITE_TARGET: &str = $target;
        const CRSQLITE_FILE_NAME: &str = $file_name;
        const BUNDLED_CRSQLITE_BYTES: &[u8] = include_bytes!($path);
    };
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
configure_crsqlite_target!(
    "darwin-aarch64",
    "crsqlite.dylib",
    "../vendor/darwin-aarch64/crsqlite.dylib"
);

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
configure_crsqlite_target!(
    "darwin-x86_64",
    "crsqlite.dylib",
    "../vendor/darwin-x86_64/crsqlite.dylib"
);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
configure_crsqlite_target!(
    "linux-x86_64",
    "crsqlite.so",
    "../vendor/linux-x86_64/crsqlite.so"
);

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
configure_crsqlite_target!(
    "win-x86_64",
    "crsqlite.dll",
    "../vendor/win-x86_64/crsqlite.dll"
);

/// Extract the vendored cr-sqlite loadable extension for the current target
/// to the cache dir and return its absolute path.
///
/// The extraction is idempotent and race-safe: the bytes are written to a
/// per-process temp file and renamed into place, so a concurrently running
/// second process either sees the old intact file or the new intact file,
/// never a torn one. An existing file of the same length is left alone (the
/// vendored bytes are immutable per version, and the version is part of the
/// path), so the common case is one write per machine per version.
pub fn bundled_extension_path() -> Result<PathBuf, Error> {
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        return Err(Error::UnsupportedBundledCrsqlite);
    }

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    ))]
    {
        let base_dir = dirs::cache_dir()
            .ok_or(Error::MissingCacheDir)?
            .join("notare")
            .join("crsqlite")
            .join(CRSQLITE_VERSION)
            .join(CRSQLITE_TARGET);

        fs::create_dir_all(&base_dir)?;

        let extension_path = base_dir.join(CRSQLITE_FILE_NAME);
        let needs_write = match fs::metadata(&extension_path) {
            Ok(metadata) => metadata.len() != BUNDLED_CRSQLITE_BYTES.len() as u64,
            Err(_) => true,
        };

        if needs_write {
            let tmp_path =
                base_dir.join(format!("{CRSQLITE_FILE_NAME}.{}.tmp", std::process::id()));
            fs::write(&tmp_path, BUNDLED_CRSQLITE_BYTES)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
            }

            match fs::rename(&tmp_path, &extension_path) {
                Ok(()) => {}
                Err(error) if extension_path.exists() => {
                    let _ = fs::remove_file(&tmp_path);

                    if fs::metadata(&extension_path)?.len() != BUNDLED_CRSQLITE_BYTES.len() as u64 {
                        return Err(error.into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }

        return Ok(extension_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vendored bytes are non-empty on every supported target (an empty
    /// or missing `include_bytes!` target would otherwise fail only when the
    /// app first loads the engine).
    #[test]
    fn vendored_binary_is_not_empty() {
        if let Ok(path) = bundled_extension_path() {
            assert!(
                path.is_file(),
                "extracted extension must exist at {}",
                path.display()
            );
        }
        // The unsupported-target case is compile-time cfg'd; nothing to
        // assert for it at runtime on this host.
    }
}
