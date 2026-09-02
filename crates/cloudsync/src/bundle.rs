#[cfg(not(feature = "from-source"))]
use std::fs;
use std::path::PathBuf;

#[cfg(not(feature = "from-source"))]
use crate::CLOUDSYNC_VERSION;
use crate::error::Error;

#[cfg(not(feature = "from-source"))]
macro_rules! configure_cloudsync_target {
    ($target:literal, $file_name:literal, $path:literal) => {
        const CLOUDSYNC_TARGET: &str = $target;
        const CLOUDSYNC_FILE_NAME: &str = $file_name;
        const BUNDLED_CLOUDSYNC_BYTES: &[u8] = include_bytes!($path);
    };
}

#[cfg(all(
    not(feature = "from-source"),
    target_os = "macos",
    target_arch = "aarch64"
))]
configure_cloudsync_target!(
    "macos/aarch64",
    "cloudsync.dylib",
    "../vendor/cloudsync/macos/aarch64/cloudsync.dylib"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "macos",
    target_arch = "x86_64"
))]
configure_cloudsync_target!(
    "macos/x86_64",
    "cloudsync.dylib",
    "../vendor/cloudsync/macos/x86_64/cloudsync.dylib"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "android",
    target_arch = "aarch64"
))]
configure_cloudsync_target!(
    "android/arm64-v8a",
    "cloudsync.so",
    "../vendor/cloudsync/android/arm64-v8a/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "android",
    target_arch = "arm"
))]
configure_cloudsync_target!(
    "android/armeabi-v7a",
    "cloudsync.so",
    "../vendor/cloudsync/android/armeabi-v7a/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "android",
    target_arch = "x86_64"
))]
configure_cloudsync_target!(
    "android/x86_64",
    "cloudsync.so",
    "../vendor/cloudsync/android/x86_64/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "linux",
    target_env = "gnu",
    target_arch = "aarch64"
))]
configure_cloudsync_target!(
    "linux/gnu/aarch64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/gnu/aarch64/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "linux",
    target_env = "gnu",
    target_arch = "x86_64"
))]
configure_cloudsync_target!(
    "linux/gnu/x86_64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/gnu/x86_64/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "linux",
    target_env = "musl",
    target_arch = "aarch64"
))]
configure_cloudsync_target!(
    "linux/musl/aarch64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/musl/aarch64/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "linux",
    target_env = "musl",
    target_arch = "x86_64"
))]
configure_cloudsync_target!(
    "linux/musl/x86_64",
    "cloudsync.so",
    "../vendor/cloudsync/linux/musl/x86_64/cloudsync.so"
);

#[cfg(all(
    not(feature = "from-source"),
    target_os = "windows",
    target_arch = "x86_64"
))]
configure_cloudsync_target!(
    "windows/x86_64",
    "cloudsync.dll",
    "../vendor/cloudsync/windows/x86_64/cloudsync.dll"
);

/// Locate the extension staged into the app bundle by `build.rs`, if present.
///
/// Probes relative to the running executable, mirroring how
/// `bundled_ios_framework_path()` below already resolves its framework. Tauri
/// puts `resources` next to the binary on Windows and Linux, and under
/// `Contents/Resources` in a macOS `.app`, so both shapes are checked.
///
/// Returns `None` when nothing is staged — the dev/test case, where the caller
/// falls back to the compile-time `OUT_DIR` path.
#[cfg(feature = "from-source")]
fn packaged_from_source_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    packaged_from_source_path_in(exe.parent()?)
}

/// The probe itself, split out from `current_exe()` so it is testable against
/// a synthetic bundle layout.
#[cfg(feature = "from-source")]
fn packaged_from_source_path_in(dir: &std::path::Path) -> Option<PathBuf> {
    let file_name = if cfg!(target_os = "windows") {
        "cloudsync.dll"
    } else if cfg!(target_os = "macos") {
        "cloudsync.dylib"
    } else {
        "cloudsync.so"
    };

    let candidates = [
        // Windows / Linux: resources land beside the executable.
        dir.join("resources").join("cloudsync").join(file_name),
        dir.join(file_name),
        // macOS .app: Contents/MacOS/<bin> → Contents/Resources/...
        dir.parent()
            .map(|c| c.join("Resources").join("cloudsync").join(file_name))
            .unwrap_or_default(),
        dir.parent()
            .map(|c| c.join("Resources").join(file_name))
            .unwrap_or_default(),
    ];

    candidates.into_iter().find(|path| path.is_file())
}

pub fn bundled_extension_path() -> Result<PathBuf, Error> {
    // From-source build: return the freshly-built .so from OUT_DIR (set by
    // build.rs via `cargo:rustc-env`). Only linux/x86_64 is supported (S0b).
    //
    // SYNC-5: baked in with `env!` (compile-time), NOT `std::env::var`
    // (runtime). cargo injects a build script's `cargo:rustc-env` into the
    // *rustc process compiling that crate*, where `env!` captures it —
    // but does NOT inject it into downstream binaries at runtime, so a
    // runtime read made every from-source consumer that is not the
    // cloudsync crate's own test binary fail with
    // `UnsupportedBundledCloudsync` (the desktop app included).
    #[cfg(feature = "from-source")]
    {
        // The `env!` path above is the BUILD MACHINE's OUT_DIR. That is correct
        // for `cargo run`/`cargo test` on that machine and wrong everywhere
        // else: OUT_DIR is not packaged, so on an installed copy the path does
        // not exist and the extension cannot load. Prefer the staged copy that
        // build.rs puts into the bundle (see `stage_for_bundling`), and fall
        // back to OUT_DIR for the dev/test case.
        if let Some(packaged) = packaged_from_source_path() {
            return Ok(packaged);
        }

        return Ok(PathBuf::from(env!(
            "CLOUDSYNC_FROM_SOURCE_SO",
            "from-source builds must be compiled through crates/cloudsync/build.rs"
        )));
    }

    #[cfg(not(feature = "from-source"))]
    {
        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "ios", target_arch = "aarch64"),
            all(target_os = "ios", target_arch = "x86_64"),
            all(target_os = "android", target_arch = "aarch64"),
            all(target_os = "android", target_arch = "arm"),
            all(target_os = "android", target_arch = "x86_64"),
            all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
            all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
            all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
            all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86_64"),
        )))]
        {
            return Err(Error::UnsupportedBundledCloudsync);
        }

        #[cfg(any(
            all(target_os = "ios", target_arch = "aarch64"),
            all(target_os = "ios", target_arch = "x86_64"),
        ))]
        {
            return bundled_ios_framework_path();
        }

        #[cfg(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "android", target_arch = "aarch64"),
            all(target_os = "android", target_arch = "arm"),
            all(target_os = "android", target_arch = "x86_64"),
            all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
            all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
            all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
            all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86_64"),
        ))]
        {
            let base_dir = dirs::cache_dir()
                .ok_or(Error::MissingCacheDir)?
                .join("char")
                .join("cloudsync")
                .join(CLOUDSYNC_VERSION)
                .join(CLOUDSYNC_TARGET);

            fs::create_dir_all(&base_dir)?;

            let extension_path = base_dir.join(CLOUDSYNC_FILE_NAME);
            let needs_write = match fs::metadata(&extension_path) {
                Ok(metadata) => metadata.len() != BUNDLED_CLOUDSYNC_BYTES.len() as u64,
                Err(_) => true,
            };

            if needs_write {
                let tmp_path =
                    base_dir.join(format!("{CLOUDSYNC_FILE_NAME}.{}.tmp", std::process::id()));
                fs::write(&tmp_path, BUNDLED_CLOUDSYNC_BYTES)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;

                    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
                }

                match fs::rename(&tmp_path, &extension_path) {
                    Ok(()) => {}
                    Err(error) if extension_path.exists() => {
                        let _ = fs::remove_file(&tmp_path);

                        if fs::metadata(&extension_path)?.len()
                            != BUNDLED_CLOUDSYNC_BYTES.len() as u64
                        {
                            return Err(error.into());
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            return Ok(extension_path);
        }
    }
}

#[cfg(any(
    all(target_os = "ios", target_arch = "aarch64"),
    all(target_os = "ios", target_arch = "x86_64"),
))]
fn bundled_ios_framework_path() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("CLOUDSYNC_IOS_FRAMEWORK_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe()?;
    let candidates = [
        exe.parent()
            .map(|dir| dir.join("Frameworks/CloudSync.framework/CloudSync")),
        exe.parent()
            .and_then(|dir| dir.parent())
            .map(|dir| dir.join("Frameworks/CloudSync.framework/CloudSync")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(Error::UnsupportedBundledCloudsync)
}

#[cfg(all(test, feature = "from-source"))]
mod tests {
    use super::*;

    fn staged_name() -> &'static str {
        if cfg!(target_os = "windows") {
            "cloudsync.dll"
        } else if cfg!(target_os = "macos") {
            "cloudsync.dylib"
        } else {
            "cloudsync.so"
        }
    }

    /// Regression: a from-source build resolved the extension only from the
    /// BUILD MACHINE's OUT_DIR, so every packaged install pointed at a path
    /// that did not exist on the user's disk. The app then could not open its
    /// database and died before logging anything.
    #[test]
    fn from_source_extension_resolves_from_a_packaged_layout_not_only_out_dir() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path();

        assert!(
            packaged_from_source_path_in(exe_dir).is_none(),
            "nothing staged yet, so the probe must fall through to OUT_DIR"
        );

        let staged = exe_dir.join("resources").join("cloudsync");
        std::fs::create_dir_all(&staged).unwrap();
        let file = staged.join(staged_name());
        std::fs::write(&file, b"not a real extension").unwrap();

        assert_eq!(
            packaged_from_source_path_in(exe_dir).as_deref(),
            Some(file.as_path()),
            "a staged extension beside the executable must win over OUT_DIR"
        );
    }

    /// macOS bundles put resources in `Contents/Resources`, a sibling of the
    /// `Contents/MacOS` directory the binary runs from.
    #[test]
    fn from_source_extension_resolves_from_a_macos_app_layout() {
        let dir = tempfile::tempdir().unwrap();
        let contents = dir.path().join("Contents");
        let macos = contents.join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();

        let resources = contents.join("Resources").join("cloudsync");
        std::fs::create_dir_all(&resources).unwrap();
        let file = resources.join(staged_name());
        std::fs::write(&file, b"not a real extension").unwrap();

        assert_eq!(
            packaged_from_source_path_in(&macos).as_deref(),
            Some(file.as_path()),
            "a .app layout must resolve through Contents/Resources"
        );
    }
}
