use std::path::{Path, PathBuf};

pub const VAULT_CONFIG_FILENAME: &str = "global.json";
const STAGING_BUNDLE_IDS: &[&str] = &["be.abhishek.notare.staging", "com.hyprnote.staging"];
const RELEASE_APP_FOLDER: &str = "notare";
/// Older release-channel data folders, newest rename first.
///
/// Migration (updated 2026-07-20): if a legacy folder still holds data and the
/// current one doesn't, rename it to `notare` — an atomic, same-volume rename
/// (safer than a copy) so the on-disk folder matches the brand instead of
/// silently staying `anarlog` forever. If the rename can't happen (target
/// exists, cross-volume, permissions) we fall back to adopting the legacy
/// folder in place so a user's data is never clobbered or lost. Fresh installs
/// land directly in `notare`.
const LEGACY_RELEASE_APP_FOLDERS: &[&str] = &["anarlog", "hyprnote"];

pub fn compute_vault_config_path(base: &Path) -> PathBuf {
    base.join(VAULT_CONFIG_FILENAME)
}

pub fn compute_default_base(bundle_id: &str) -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let app_folder = resolve_app_folder(&data_dir, bundle_id, cfg!(debug_assertions));
    Some(data_dir.join(app_folder))
}

/// Resolve the app-data folder name, migrating a legacy folder to the current
/// name in place when one is found (see [`LEGACY_RELEASE_APP_FOLDERS`]).
fn resolve_app_folder<'a>(data_dir: &Path, bundle_id: &'a str, is_debug: bool) -> &'a str {
    if is_debug || STAGING_BUNDLE_IDS.contains(&bundle_id) {
        return bundle_id;
    }
    if has_app_data(&data_dir.join(RELEASE_APP_FOLDER)) {
        return RELEASE_APP_FOLDER;
    }
    match LEGACY_RELEASE_APP_FOLDERS
        .iter()
        .copied()
        .find(|folder| has_app_data(&data_dir.join(folder)))
    {
        // Renamed the legacy folder to `notare`; use the new name.
        Some(legacy) if migrate_legacy_folder(data_dir, legacy) => RELEASE_APP_FOLDER,
        // Rename couldn't happen — adopt the legacy folder in place (data safe).
        Some(legacy) => legacy,
        None => RELEASE_APP_FOLDER,
    }
}

/// Rename a legacy data folder to the current `notare` folder. Same-volume
/// rename is atomic. Returns `false` (caller adopts in place) when the target
/// already exists or the rename fails, so data is never clobbered or lost.
fn migrate_legacy_folder(data_dir: &Path, legacy: &str) -> bool {
    let to = data_dir.join(RELEASE_APP_FOLDER);
    if to.exists() {
        return false;
    }
    std::fs::rename(data_dir.join(legacy), &to).is_ok()
}

fn has_app_data(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or_else(|_| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_app_folder_uses_notare_for_new_stable_installs() {
        let temp = tempdir().unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_migrates_anarlog_to_notare() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join("anarlog");
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
        // The legacy folder was renamed: anarlog is gone, notare holds the data.
        assert!(!temp.path().join("anarlog").exists());
        assert!(
            temp.path()
                .join(RELEASE_APP_FOLDER)
                .join("store.json")
                .exists()
        );
    }

    #[test]
    fn resolve_app_folder_migrates_hyprnote_to_notare() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join("hyprnote");
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
        assert!(
            temp.path()
                .join(RELEASE_APP_FOLDER)
                .join("store.json")
                .exists()
        );
    }

    #[test]
    fn resolve_app_folder_migrates_anarlog_before_hyprnote() {
        let temp = tempdir().unwrap();
        for folder in ["anarlog", "hyprnote"] {
            let base = temp.path().join(folder);
            std::fs::create_dir_all(&base).unwrap();
            std::fs::write(base.join(format!("{folder}.json")), "{}").unwrap();
        }

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
        // anarlog (first in the list) migrated into notare; hyprnote untouched.
        assert!(
            temp.path()
                .join(RELEASE_APP_FOLDER)
                .join("anarlog.json")
                .exists()
        );
        assert!(temp.path().join("hyprnote").exists());
    }

    #[test]
    fn resolve_app_folder_adopts_legacy_in_place_when_notare_dir_blocks_rename() {
        let temp = tempdir().unwrap();
        // An empty `notare` dir exists (no data, but blocks the rename target).
        std::fs::create_dir_all(temp.path().join(RELEASE_APP_FOLDER)).unwrap();
        let legacy_base = temp.path().join("anarlog");
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        // Can't clobber the existing notare dir, so adopt anarlog in place.
        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            "anarlog"
        );
        assert!(temp.path().join("anarlog").join("store.json").exists());
    }

    #[test]
    fn resolve_app_folder_prefers_notare_when_new_folder_has_data() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join("anarlog");
        let new_base = temp.path().join(RELEASE_APP_FOLDER);
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::create_dir_all(&new_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();
        std::fs::write(new_base.join("app.db"), "").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_ignores_empty_legacy_folders() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("anarlog")).unwrap();
        std::fs::create_dir_all(temp.path().join("hyprnote")).unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_uses_notare_for_other_release_bundle_ids() {
        let temp = tempdir().unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.Notare", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_returns_bundle_id_for_staging() {
        for staging_id in STAGING_BUNDLE_IDS {
            assert_eq!(
                resolve_app_folder(Path::new("/tmp"), staging_id, false),
                *staging_id
            );
        }
    }

    #[test]
    fn resolve_app_folder_returns_bundle_id_in_debug_builds() {
        assert_eq!(
            resolve_app_folder(Path::new("/tmp"), "be.abhishek.notare.dev", true),
            "be.abhishek.notare.dev"
        );
    }
}
