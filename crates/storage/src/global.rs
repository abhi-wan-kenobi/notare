use std::path::{Path, PathBuf};

pub const VAULT_CONFIG_FILENAME: &str = "global.json";
const STAGING_BUNDLE_IDS: &[&str] = &["be.abhishek.notare.staging", "com.hyprnote.staging"];
const RELEASE_APP_FOLDER: &str = "notare";
/// Older release-channel data folders, newest rename first.
///
/// Migration decision (2026-07-16): if a legacy folder still holds data and
/// the current one doesn't, we keep using the legacy folder *in place* — no
/// copy/move. That is the safest minimal approach: existing users' sessions
/// and models survive the rename with zero data-move risk, while fresh
/// installs land in `notare`.
const LEGACY_RELEASE_APP_FOLDERS: &[&str] = &["anarlog", "hyprnote"];

pub fn compute_vault_config_path(base: &Path) -> PathBuf {
    base.join(VAULT_CONFIG_FILENAME)
}

pub fn compute_default_base(bundle_id: &str) -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let app_folder = resolve_app_folder(&data_dir, bundle_id, cfg!(debug_assertions));
    Some(data_dir.join(app_folder))
}

fn resolve_app_folder<'a>(data_dir: &Path, bundle_id: &'a str, is_debug: bool) -> &'a str {
    if is_debug || STAGING_BUNDLE_IDS.contains(&bundle_id) {
        return bundle_id;
    }
    if has_app_data(&data_dir.join(RELEASE_APP_FOLDER)) {
        return RELEASE_APP_FOLDER;
    }
    LEGACY_RELEASE_APP_FOLDERS
        .iter()
        .copied()
        .find(|folder| has_app_data(&data_dir.join(folder)))
        .unwrap_or(RELEASE_APP_FOLDER)
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
    fn resolve_app_folder_keeps_anarlog_folder_when_it_has_data() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join("anarlog");
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            "anarlog"
        );
    }

    #[test]
    fn resolve_app_folder_keeps_hyprnote_folder_when_it_has_data() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join("hyprnote");
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            "hyprnote"
        );
    }

    #[test]
    fn resolve_app_folder_prefers_anarlog_over_hyprnote() {
        let temp = tempdir().unwrap();
        for folder in ["anarlog", "hyprnote"] {
            let base = temp.path().join(folder);
            std::fs::create_dir_all(&base).unwrap();
            std::fs::write(base.join("store.json"), "{}").unwrap();
        }

        assert_eq!(
            resolve_app_folder(temp.path(), "be.abhishek.notare", false),
            "anarlog"
        );
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
