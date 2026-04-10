use std::path::PathBuf;

use tauri::{Manager, Runtime};

fn bundled_resource_candidates(relative_path: &str) -> Vec<String> {
    let mut candidates = vec![relative_path.to_string()];
    if cfg!(debug_assertions) {
        candidates.push(format!("resources/{relative_path}"));
    }
    candidates
}

#[cfg(debug_assertions)]
fn source_tree_resource_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src-tauri/resources")
        .join(relative_path)
}

pub(crate) fn resolve_resource_path<R: Runtime, T: Manager<R>>(
    manager: &T,
    relative_path: &str,
) -> Result<Option<PathBuf>, hypr_local_llm_core::Error> {
    use tauri::path::BaseDirectory;

    #[cfg(debug_assertions)]
    {
        let source_path = source_tree_resource_path(relative_path);
        if source_path.exists() {
            return Ok(Some(source_path));
        }
    }

    for candidate in bundled_resource_candidates(relative_path) {
        let path = manager
            .path()
            .resolve(&candidate, BaseDirectory::Resource)
            .map_err(|error| hypr_local_llm_core::Error::Other(error.to_string()))?;
        if path.exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn resolve_embedded_llm_args<R: Runtime, T: Manager<R>>(
    manager: &T,
) -> Result<(String, PathBuf), crate::Error> {
    let model = hypr_local_model::CactusLlmModel::Lfm2Vl450mApple;
    let hypr_local_model::CactusModelSource::BundledResource { relative_path } = model.source()
    else {
        return Err(crate::Error::Other(format!(
            "embedded local LLM resource is unavailable for {}",
            model.asset_id()
        )));
    };
    let model_path = resolve_resource_path(manager, relative_path)?.ok_or_else(|| {
        crate::Error::Other(format!(
            "embedded local LLM resource not found: {relative_path}"
        ))
    })?;
    tracing::info!(
        model = %model.asset_id(),
        path = %model_path.display(),
        "resolved_embedded_local_llm_model_path"
    );

    Ok((model.display_name().to_string(), model_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_resource_candidates_include_dev_fallback() {
        let candidates = bundled_resource_candidates("models/cactus/char-vlm/weight");

        assert_eq!(candidates[0], "models/cactus/char-vlm/weight");
        if cfg!(debug_assertions) {
            assert_eq!(candidates[1], "resources/models/cactus/char-vlm/weight");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn source_tree_resource_path_points_to_desktop_resources() {
        let path = source_tree_resource_path("models/cactus/char-vlm/weight");

        assert!(path.ends_with("apps/desktop/src-tauri/resources/models/cactus/char-vlm/weight"));
    }
}
