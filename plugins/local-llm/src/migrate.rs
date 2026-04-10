use std::path::Path;

const SPECIAL_TOKENS: &str = include_str!("../assets/special_tokens.gen.json");
const SPECIAL_TOKENS_FILE: &str = "special_tokens.json";

pub fn legacy_gguf_files(data_dir: &Path, models_dir: &Path) {
    let _ = std::fs::create_dir_all(models_dir);

    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("gguf") {
                if let Some(name) = path.file_name() {
                    let _ = std::fs::rename(&path, models_dir.join(name));
                }
            }
        }
    }
}

pub fn whisper_small_special_tokens(cactus_models_dir: &Path) -> crate::Result<()> {
    if !cactus_models_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(cactus_models_dir)? {
        let path = entry?.path();

        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !name.starts_with("whisper-small") {
            continue;
        }

        let target = path.join(SPECIAL_TOKENS_FILE);
        if std::fs::read_to_string(&target).ok().as_deref() == Some(SPECIAL_TOKENS) {
            continue;
        }

        std::fs::write(&target, SPECIAL_TOKENS)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_only_whisper_small_model_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let cactus_models_dir = temp.path().join("cactus");
        std::fs::create_dir_all(cactus_models_dir.join("whisper-small-int8-apple")).unwrap();
        std::fs::create_dir_all(cactus_models_dir.join("whisper-medium-int8-apple")).unwrap();

        whisper_small_special_tokens(&cactus_models_dir).unwrap();

        let patched = std::fs::read_to_string(
            cactus_models_dir
                .join("whisper-small-int8-apple")
                .join(SPECIAL_TOKENS_FILE),
        )
        .unwrap();
        assert_eq!(patched, SPECIAL_TOKENS);

        assert!(
            !cactus_models_dir
                .join("whisper-medium-int8-apple")
                .join(SPECIAL_TOKENS_FILE)
                .exists()
        );
    }

    #[test]
    fn overwrites_stale_special_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let model_dir = temp.path().join("cactus").join("whisper-small-int8-apple");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join(SPECIAL_TOKENS_FILE), "{}").unwrap();

        whisper_small_special_tokens(temp.path().join("cactus").as_path()).unwrap();

        let patched = std::fs::read_to_string(model_dir.join(SPECIAL_TOKENS_FILE)).unwrap();
        assert_eq!(patched, SPECIAL_TOKENS);
    }
}
