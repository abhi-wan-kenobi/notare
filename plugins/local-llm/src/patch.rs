use std::path::Path;

const SPECIAL_TOKENS: &str = include_str!("../assets/special_tokens.gen.json");
const SPECIAL_TOKENS_FILE: &str = "special_tokens.json";

pub fn apply_whisper_small_special_tokens(cactus_models_dir: &Path) -> crate::Result<()> {
    if !cactus_models_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(cactus_models_dir)
        .map_err(|error| crate::Error::Other(error.to_string()))?
    {
        let entry = entry.map_err(|error| crate::Error::Other(error.to_string()))?;
        let path = entry.path();

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

        std::fs::write(&target, SPECIAL_TOKENS)
            .map_err(|error| crate::Error::Other(error.to_string()))?;
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

        apply_whisper_small_special_tokens(&cactus_models_dir).unwrap();

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

        apply_whisper_small_special_tokens(temp.path().join("cactus").as_path()).unwrap();

        let patched = std::fs::read_to_string(model_dir.join(SPECIAL_TOKENS_FILE)).unwrap();
        assert_eq!(patched, SPECIAL_TOKENS);
    }
}
