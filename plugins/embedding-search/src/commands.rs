use tauri::ipc::Channel;

use crate::runtime::DownloadProgress;
use crate::{ChunkInput, IndexStatus, ManagedState, SearchHit};

/// Download-on-first-run: fetch the pinned EmbeddingGemma artifacts into the
/// model dir, streaming SHA-256-verified progress over `on_progress`. Idempotent.
#[tauri::command]
#[specta::specta]
pub(crate) async fn download_embedding_model(
    state: tauri::State<'_, ManagedState>,
    on_progress: Channel<DownloadProgress>,
) -> Result<(), String> {
    state
        .download_model(move |p| {
            let _ = on_progress.send(p);
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn embed_and_index_chunks(
    state: tauri::State<'_, ManagedState>,
    session_id: String,
    chunks: Vec<ChunkInput>,
) -> Result<u32, String> {
    state
        .embed_and_index_chunks(session_id, chunks)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_session_chunks(
    state: tauri::State<'_, ManagedState>,
    session_id: String,
) -> Result<u32, String> {
    state
        .delete_session_chunks(session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn semantic_search(
    state: tauri::State<'_, ManagedState>,
    query: String,
    k: u32,
    session_id: Option<String>,
) -> Result<Vec<SearchHit>, String> {
    state
        .semantic_search(query, k, session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn embedding_index_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<IndexStatus, String> {
    state.index_status().await.map_err(|e| e.to_string())
}
