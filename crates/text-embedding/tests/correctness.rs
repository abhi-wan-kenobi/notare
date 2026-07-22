//! Model-gated correctness tests (S0 → B1 merge gate).
//!
//! These need the ~330 MB EmbeddingGemma artifacts on disk, so they are
//! `#[ignore]` by default. Run them with the model dir set:
//!
//! ```sh
//! NOTARE_TEXT_EMBEDDING_MODEL_DIR=/path/to/egemma cargo test -p text-embedding -- --ignored
//! ```
//!
//! The reference fixture (`tests/fixtures/reference_embeddings.json`) was
//! generated on 2026-07-23 by an independent Python pipeline (onnxruntime +
//! HF `tokenizers`) over the same pinned ONNX export — see the crate README
//! for the generation script and the acceptance thresholds.

use text_embedding::{EMBEDDING_DIM, TextEmbedder};

#[derive(serde::Deserialize)]
struct Fixture {
    query_prefix: String,
    doc_prefix: String,
    sentences: Vec<String>,
    doc_embeddings_768: Vec<Vec<f32>>,
    queries: Vec<String>,
    query_embeddings_768: Vec<Vec<f32>>,
    rank_top3: std::collections::HashMap<String, Vec<usize>>,
}

fn model_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("NOTARE_TEXT_EMBEDDING_MODEL_DIR").map(Into::into)
}

fn load_fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/reference_embeddings.json")).unwrap()
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    dot / (na * nb)
}

fn trunc512(v: &[f32]) -> Vec<f32> {
    let mut t = v[..EMBEDDING_DIM].to_vec();
    let n: f64 = t.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    for x in &mut t {
        *x = ((*x as f64) / n) as f32;
    }
    t
}

/// Gate 1: per-sentence cosine >= 0.99 against the independent reference
/// pipeline, for both document and query embeddings.
#[test]
#[ignore = "needs NOTARE_TEXT_EMBEDDING_MODEL_DIR with the ~330MB model artifacts"]
fn embeddings_match_reference_pipeline() {
    let dir = model_dir().expect("set NOTARE_TEXT_EMBEDDING_MODEL_DIR");
    let fx = load_fixture();
    let mut embedder = TextEmbedder::load(&dir).unwrap();

    let sents: Vec<&str> = fx.sentences.iter().map(String::as_str).collect();
    let ours = embedder.embed_docs(&sents).unwrap();
    assert_eq!(ours.len(), fx.doc_embeddings_768.len());
    for (i, (mine, reference)) in ours.iter().zip(&fx.doc_embeddings_768).enumerate() {
        assert_eq!(mine.len(), EMBEDDING_DIM);
        let c = cosine(mine, &trunc512(reference));
        assert!(
            c >= 0.99,
            "doc {i} cosine {c:.5} < 0.99: {:?}",
            fx.sentences[i]
        );
    }

    for (i, q) in fx.queries.iter().enumerate() {
        let mine = embedder.embed_query(q).unwrap();
        let c = cosine(&mine, &trunc512(&fx.query_embeddings_768[i]));
        assert!(c >= 0.99, "query {i} cosine {c:.5} < 0.99: {q:?}");
    }
}

/// Gate 2: top-3 retrieval rank agreement with the reference pipeline on the
/// 10-doc x 5-query set.
#[test]
#[ignore = "needs NOTARE_TEXT_EMBEDDING_MODEL_DIR with the ~330MB model artifacts"]
fn top3_rank_agreement_with_reference() {
    let dir = model_dir().expect("set NOTARE_TEXT_EMBEDDING_MODEL_DIR");
    let fx = load_fixture();
    let mut embedder = TextEmbedder::load(&dir).unwrap();

    let docs: Vec<&str> = fx.sentences[..10].iter().map(String::as_str).collect();
    let doc_vecs = embedder.embed_docs(&docs).unwrap();

    for q in &fx.queries {
        let qv = embedder.embed_query(q).unwrap();
        let mut order: Vec<usize> = (0..docs.len()).collect();
        order.sort_by(|&a, &b| {
            cosine(&qv, &doc_vecs[b])
                .partial_cmp(&cosine(&qv, &doc_vecs[a]))
                .unwrap()
        });
        let expected = &fx.rank_top3[q];
        assert_eq!(
            &order[..3],
            &expected[..],
            "top-3 disagreement for query {q:?}"
        );
    }
}

/// Gate 3 (prefix trap): the same text embedded as a query and as a document
/// must NOT produce the same vector. Guards against the prefixes being lost
/// or unified in a refactor.
#[test]
#[ignore = "needs NOTARE_TEXT_EMBEDDING_MODEL_DIR with the ~330MB model artifacts"]
fn query_and_doc_embeddings_differ_for_same_text() {
    let dir = model_dir().expect("set NOTARE_TEXT_EMBEDDING_MODEL_DIR");
    let fx = load_fixture();
    let mut embedder = TextEmbedder::load(&dir).unwrap();

    let text = fx.sentences[0].as_str();
    let q = embedder.embed_query(text).unwrap();
    let d = embedder.embed_docs(&[text]).unwrap().remove(0);
    let c = cosine(&q, &d);
    assert!(
        c < 0.99,
        "prefix trap: query and doc embeddings of the same text are near-identical \
         (cosine {c:.5}) — prompt prefixes are not being applied"
    );
}

/// Gate 4: artifact integrity verification catches the happy path.
#[test]
#[ignore = "needs NOTARE_TEXT_EMBEDDING_MODEL_DIR with the ~330MB model artifacts"]
fn artifacts_verify_against_pinned_hashes() {
    let dir = model_dir().expect("set NOTARE_TEXT_EMBEDDING_MODEL_DIR");
    text_embedding::verify_artifacts(&dir).unwrap();
}

/// The fixture's recorded prefixes must equal the crate's (compile-time pinned
/// in unit tests; this cross-checks the FIXTURE was generated with them).
#[test]
fn fixture_was_generated_with_the_contract_prefixes() {
    let fx = load_fixture();
    assert_eq!(fx.query_prefix, "task: search result | query: ");
    assert_eq!(fx.doc_prefix, "title: none | text: ");
}
