//! Typed Rust entry point into semantic search for other plugins/crates that
//! share the same `AppHandle` tree (e.g. `plugins/mcp-server`'s hybrid
//! `search_meeting_content` tool). Calling [`EmbeddingSearchRuntime::semantic_search`]
//! through this trait reuses the exact code path backing the
//! `semantic_search` Tauri command in `commands.rs` — there is only one
//! implementation of the query.

use std::sync::Arc;

use tauri::Manager;

use crate::{EmbeddingSearchRuntime, ManagedState};

/// Access to the embedding-search runtime from an `AppHandle` (or any other
/// `tauri::Manager`, e.g. in tests). Returns `None` when the `embedding-search`
/// plugin was never registered on this app — callers must treat that the same
/// as "semantic search unavailable" and fall back rather than panicking on a
/// missing managed state.
pub trait EmbeddingSearchPluginExt<R: tauri::Runtime> {
    fn embedding_search(&self) -> Option<Arc<EmbeddingSearchRuntime>>;
}

impl<R: tauri::Runtime, T: Manager<R>> EmbeddingSearchPluginExt<R> for T {
    fn embedding_search(&self) -> Option<Arc<EmbeddingSearchRuntime>> {
        self.try_state::<ManagedState>().map(|s| s.inner().clone())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::EmbeddingSearchPluginExt;

    fn mock_ctx() -> tauri::Context<tauri::test::MockRuntime> {
        let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
        ctx.config_mut().identifier = "com.hyprnote.dev".to_string();
        ctx.config_mut().version = Some("0.0.1".to_string());
        ctx
    }

    #[test]
    fn returns_none_when_plugin_is_not_registered() {
        let app = tauri::test::mock_builder().build(mock_ctx()).unwrap();
        assert!(app.handle().embedding_search().is_none());
    }

    #[tokio::test]
    async fn returns_the_managed_runtime_when_plugin_is_registered() {
        let db = Arc::new(hypr_db_core::Db::connect_memory_plain().await.unwrap());
        let app = tauri::test::mock_builder()
            .plugin(crate::init::<tauri::test::MockRuntime>(
                db,
                PathBuf::from("/nonexistent-model-dir"),
            ))
            .build(mock_ctx())
            .unwrap();

        let runtime = app
            .handle()
            .embedding_search()
            .expect("plugin registered, runtime must be managed");
        // The extension trait reuses the real runtime: status is queryable.
        let status = runtime.index_status().await.unwrap();
        assert_eq!(status.chunk_count, 0);
        assert!(!status.model_downloaded);
    }
}
