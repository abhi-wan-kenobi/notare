//! On-device semantic search plugin (Notare 0.5, WS-B1 PR9).
//!
//! Indexes note/transcript text chunks as 512-d EmbeddingGemma vectors into the
//! `embedding_*` tables (see the `20260723120000_embedding_chunks` migration)
//! and answers KNN semantic queries via sqlite-vec's `vec0` virtual table.
//!
//! The model is loaded lazily on the first embed call and reused for the
//! process lifetime; the ~330 MB weights are downloaded on first run and are
//! NOT required for the plugin to build, start, or report status.

mod commands;
mod error;
mod runtime;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::Manager;

pub use error::{Error, Result};
pub use runtime::EmbeddingSearchRuntime;

const PLUGIN_NAME: &str = "embedding-search";

pub type ManagedState = Arc<EmbeddingSearchRuntime>;

/// A single text chunk to embed and index.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInput {
    /// The chunk text to embed.
    pub text: String,
    /// `'note'` | `'transcript'` — which surface this chunk came from.
    pub source_type: String,
    /// Word-timing anchor (ms into the recording) for transcript chunks; `None`
    /// for note chunks.
    pub start_ms: Option<i64>,
    /// Hash of the source text at index time; drives idempotent re-index.
    pub content_hash: String,
}

/// One semantic search result, chunk metadata plus its distance to the query.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chunk_id: String,
    pub session_id: String,
    pub source_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
    /// sqlite-vec L2 distance; smaller is closer.
    pub distance: f32,
}

/// Reported by `embedding_index_status`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    /// Whether the model artifacts required to embed are present on disk.
    pub model_downloaded: bool,
    /// Number of chunks currently indexed.
    pub chunk_count: i64,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::embed_and_index_chunks,
            commands::delete_session_chunks,
            commands::semantic_search,
            commands::embedding_index_status,
            commands::download_embedding_model,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>(
    db: Arc<hypr_db_core::Db>,
    model_dir: PathBuf,
) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            hypr_tauri_utils::block_on(hypr_db_app::prepare_schema(db.as_ref()))?;
            app.manage(Arc::new(EmbeddingSearchRuntime::new(db, model_dir)));
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::sync::Arc;

    use hypr_text_embedding::EMBEDDING_DIM;

    use super::runtime::{EmbeddingSearchRuntime, PreparedChunk};
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    /// Build a unit vector with `1.0` in slot `i` (a distinct, orthonormal
    /// basis vector so nearest-neighbour distances are known exactly).
    fn one_hot(i: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; EMBEDDING_DIM];
        v[i] = 1.0;
        v
    }

    fn prepared(
        text: &str,
        source_type: &str,
        start_ms: Option<i64>,
        vector: Vec<f32>,
    ) -> PreparedChunk {
        PreparedChunk {
            input: ChunkInput {
                text: text.to_string(),
                source_type: source_type.to_string(),
                start_ms,
                // content_hash must be unique per chunk or chunk_id collides.
                content_hash: format!("hash-{text}"),
            },
            vector,
        }
    }

    async fn runtime() -> EmbeddingSearchRuntime {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        // A directory with no model files: status must report model absent, and
        // the storage/KNN path must still work with supplied vectors.
        EmbeddingSearchRuntime::new(Arc::new(db), PathBuf::from("/nonexistent-model-dir"))
    }

    #[tokio::test]
    async fn stores_and_knn_ranks_by_distance() {
        let rt = runtime().await;

        let inserted = rt
            .store_chunk_vectors(
                "session-1",
                vec![
                    prepared("apple", "note", None, one_hot(0)),
                    prepared("banana", "note", None, one_hot(1)),
                    prepared("cherry", "transcript", Some(1234), one_hot(2)),
                ],
            )
            .await
            .unwrap();
        assert_eq!(inserted, 3);

        // Query exactly at banana's vector: it must rank first (distance ~0).
        let hits = rt.knn_join(&one_hot(1), 5, None).await.unwrap();
        assert_eq!(hits.len(), 3, "k larger than corpus returns whole corpus");
        assert_eq!(hits[0].text, "banana");
        assert!(hits[0].distance <= hits[1].distance);
        assert!(hits[0].distance < 1e-4, "exact match distance ~0");

        // transcript chunk carried its start_ms through the round-trip.
        let cherry = hits.iter().find(|h| h.text == "cherry").unwrap();
        assert_eq!(cherry.start_ms, Some(1234));
        assert_eq!(cherry.source_type, "transcript");

        // status reflects the corpus and reports the model absent.
        let status = rt.index_status().await.unwrap();
        assert_eq!(status.chunk_count, 3);
        assert!(!status.model_downloaded);
    }

    #[tokio::test]
    async fn reindex_is_idempotent_per_content_hash() {
        let rt = runtime().await;
        let batch = || {
            vec![
                prepared("apple", "note", None, one_hot(0)),
                prepared("banana", "note", None, one_hot(1)),
            ]
        };

        assert_eq!(rt.store_chunk_vectors("s", batch()).await.unwrap(), 2);
        // Same content again: nothing new is inserted.
        assert_eq!(rt.store_chunk_vectors("s", batch()).await.unwrap(), 0);
        assert_eq!(rt.index_status().await.unwrap().chunk_count, 2);
    }

    #[tokio::test]
    async fn session_filter_restricts_results() {
        let rt = runtime().await;
        rt.store_chunk_vectors("s1", vec![prepared("apple", "note", None, one_hot(0))])
            .await
            .unwrap();
        rt.store_chunk_vectors("s2", vec![prepared("banana", "note", None, one_hot(0))])
            .await
            .unwrap();

        let all = rt.knn_join(&one_hot(0), 10, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let only_s1 = rt.knn_join(&one_hot(0), 10, Some("s1")).await.unwrap();
        assert_eq!(only_s1.len(), 1);
        assert_eq!(only_s1[0].session_id, "s1");
    }

    #[tokio::test]
    async fn delete_session_clears_chunks_and_vectors() {
        let rt = runtime().await;
        rt.store_chunk_vectors(
            "s1",
            vec![
                prepared("apple", "note", None, one_hot(0)),
                prepared("banana", "note", None, one_hot(1)),
            ],
        )
        .await
        .unwrap();

        let deleted = rt.delete_session_chunks("s1".to_string()).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(rt.index_status().await.unwrap().chunk_count, 0);
        // The vec0 rows are gone too, so a KNN over an empty index returns none.
        assert!(rt.knn_join(&one_hot(0), 5, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_query_and_zero_k_return_no_hits() {
        let rt = runtime().await;
        rt.store_chunk_vectors("s", vec![prepared("apple", "note", None, one_hot(0))])
            .await
            .unwrap();
        // Empty query short-circuits before touching the (absent) model.
        assert!(
            rt.semantic_search("   ".to_string(), 5, None)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(rt.knn_join(&one_hot(0), 0, None).await.unwrap().is_empty());
    }
}
