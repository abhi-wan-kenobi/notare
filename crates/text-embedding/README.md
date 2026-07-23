# text-embedding

On-device text embeddings for Notare's semantic search (0.5, WS-B1).
EmbeddingGemma-300M, int8 ONNX (`onnx-community/embeddinggemma-300m-ONNX`,
un-gated mirror of the Gemma-licensed model), running on the workspace's
existing `ort =2.0.0-rc.10` via `hypr-onnx`.

## S0 spike decision (2026-07-22, recorded per plan)

**GO — Option B**: run the ONNX export directly on `hypr-onnx`; no new
dependency, no new onnxruntime linkage (macOS keeps exactly the two dylibs
bundled at `ee8c22336`). Option A (fastembed) was proven lock-compatible
(fastembed 5.8.0 unifies on the pinned ort) but rejected: it pins an old
fastembed and couples upgrade cycles for zero benefit. sqlite-vec was
separately verified GO via static `sqlite3_auto_extension` registration on
sqlx `=0.9.0-alpha.1` + sqlite-unbundled (no dylib, no notarization impact).
Full evidence: vault note `2026-07-22-S0-Onnx-Embedding-Spike-Report.md`.

## Model contract (do not change casually)

- **Prompt prefixes** are applied *inside* the crate and never exported:
  - query: `task: search result | query: `
  - document: `title: none | text: `
  Same-text query-vs-doc cosine is ~0.86 — losing the prefixes silently
  corrupts retrieval. `tests/correctness.rs` gate 3 guards this.
- The graph bakes in mask-weighted mean pooling + L2 norm
  (`sentence_embedding`, 768-d unit vector). The crate Matryoshka-truncates
  to **512-d** and re-normalizes; the sqlite-vec `vec0` table is `float[512]`.
- Tokenization: `tokenizer.json`'s post-processor adds `<bos>`(2)/`<eos>`(1);
  manual zero-padding + attention mask for batches.
- Artifacts are pinned by SHA-256 in `src/manifest.rs` (download-on-first-run;
  never bundled — Gemma-terms scope decision). CRC32 + sizes are included for
  `model-downloader` `DownloadPart` integration (WS-B1 PR9).

## Testing

Unit tests run everywhere. Correctness gates need the ~330 MB artifacts:

```sh
# artifacts: model_quantized.onnx, model_quantized.onnx_data, tokenizer.json
NOTARE_TEXT_EMBEDDING_MODEL_DIR=~/models/egemma \
  cargo test -p text-embedding -- --ignored
```

Acceptance (B1 merge gate): per-sentence cosine >= 0.99 vs the independent
Python reference (onnxruntime + HF `tokenizers`, script below), top-3 rank
agreement on the 10-doc x 5-query fixture, prefix-trap inequality, SHA-256
integrity. Fixture generator: `scripts/gen_reference_fixtures.py` (kept next
to this crate; regenerate only when the pinned artifacts change).
