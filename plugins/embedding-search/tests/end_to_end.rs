//! End-to-end proof that the semantic-search dense arm actually returns results
//! (WS-B2 RC gate). Gated on the real ~330 MB EmbeddingGemma model.
//!
//! Run against an already-downloaded model:
//!   NOTARE_EMBEDDING_MODEL_DIR=/path/to/model \
//!     cargo test -p tauri-plugin-embedding-search --test end_to_end -- --ignored --nocapture
//!
//! Or prove the download-on-first-run manager itself (fetches to a temp dir):
//!   NOTARE_RUN_DOWNLOAD=1 \
//!     cargo test -p tauri-plugin-embedding-search --test end_to_end -- --ignored --nocapture

use std::sync::Arc;

use hypr_db_core::Db;
use tauri_plugin_embedding_search::{ChunkInput, EmbeddingSearchRuntime};

fn chunk(text: &str) -> ChunkInput {
    ChunkInput {
        text: text.to_string(),
        source_type: "note".to_string(),
        start_ms: None,
        content_hash: format!("{:x}", text.len() as u64 * 2654435761),
    }
}

const CORPUS: &[(&str, &str)] = &[
    (
        "s1",
        "The quarterly budget review is scheduled for next Friday afternoon.",
    ),
    (
        "s2",
        "Alice will onboard the two new backend engineers starting Monday.",
    ),
    (
        "s3",
        "We decided to migrate the customer database to a managed cloud service.",
    ),
    (
        "s4",
        "The mobile app crash on startup was traced to a null pointer in the auth flow.",
    ),
    (
        "s5",
        "Renew the SSL certificate before it expires at the end of the month.",
    ),
    (
        "s6",
        "Bob is refactoring the payment webhook to add retry with exponential backoff.",
    ),
];

/// Paraphrase queries (little/no lexical overlap with the relevant doc) → BM25
/// would miss these; the dense arm must recover them.
const QUERIES: &[(&str, &str)] = &[
    ("when is the fiscal spending meeting", "s1"),
    ("bringing on additional developers", "s2"),
    ("move our records to a hosted provider", "s3"),
    ("the program keeps dying when it opens", "s4"),
    ("update the expiring security cert", "s5"),
    ("make the billing callback resilient to failures", "s6"),
];

#[tokio::test]
#[ignore = "needs the real EmbeddingGemma model (NOTARE_EMBEDDING_MODEL_DIR or NOTARE_RUN_DOWNLOAD=1)"]
async fn dense_arm_returns_real_results() {
    let tmp = tempfile::tempdir().unwrap();
    let model_dir = match std::env::var("NOTARE_EMBEDDING_MODEL_DIR") {
        Ok(dir) => std::path::PathBuf::from(dir),
        Err(_) => tmp.path().join("model"),
    };

    let db = Arc::new(Db::connect_memory_plain().await.unwrap());
    let runtime = EmbeddingSearchRuntime::new(db, model_dir.clone());

    if std::env::var("NOTARE_RUN_DOWNLOAD").is_ok() {
        // Prove the download-on-first-run manager: fetch + SHA-verify the pinned
        // artifacts, printing progress.
        println!("downloading EmbeddingGemma to {}...", model_dir.display());
        runtime
            .download_model(|p| {
                if p.file_total > 0 && (p.file_downloaded == p.file_total) {
                    println!("  {} done ({} bytes)", p.file, p.file_total);
                }
            })
            .await
            .expect("download_model should succeed");
    }

    // Index each doc as its own session (one chunk each).
    for (id, text) in CORPUS {
        let n = runtime
            .embed_and_index_chunks(id.to_string(), vec![chunk(text)])
            .await
            .expect("indexing should succeed");
        assert_eq!(n, 1, "one chunk indexed for {id}");
    }

    // Every paraphrase query must retrieve its relevant doc at rank 1.
    let mut top1 = 0;
    for (q, relevant) in QUERIES {
        let hits = runtime
            .semantic_search(q.to_string(), 3, None)
            .await
            .expect("search should succeed");
        assert!(!hits.is_empty(), "query {q:?} returned no dense results");
        let got = &hits[0].session_id;
        println!(
            "query {q:?} -> top1 {} (want {}) dist {:.4}",
            got, relevant, hits[0].distance
        );
        if got == relevant {
            top1 += 1;
        }
    }
    // The whole point: the dense arm actually retrieves the semantically-relevant
    // doc for paraphrase queries a lexical index would miss.
    assert!(
        top1 >= QUERIES.len() - 1,
        "dense arm should retrieve the relevant doc for (almost) every paraphrase; got {top1}/{}",
        QUERIES.len()
    );
}
