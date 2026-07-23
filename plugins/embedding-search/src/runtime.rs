use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use hypr_db_core::Db;
use hypr_text_embedding::TextEmbedder;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OnceCell};

use crate::error::{Error, Result};
use crate::{ChunkInput, IndexStatus, SearchHit};

/// File names required by [`TextEmbedder::load`]. Kept as local constants so the
/// status check never needs to touch the ~330 MB weights.
const MODEL_FILE: &str = "model_quantized.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";

/// Streaming download progress, emitted per chunk over the command's Channel.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DownloadProgress {
    /// Artifact currently downloading.
    pub file: String,
    /// Bytes of THIS artifact downloaded so far.
    pub file_downloaded: u64,
    /// Total bytes of THIS artifact.
    pub file_total: u64,
    /// Bytes across ALL artifacts so far.
    pub downloaded: u64,
    /// Total bytes across all artifacts.
    pub total: u64,
}

impl DownloadProgress {
    fn new(file: &str, fd: u64, ft: u64, downloaded: u64, total: u64) -> Self {
        Self {
            file: file.to_string(),
            file_downloaded: fd,
            file_total: ft,
            downloaded,
            total,
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn sha256_file_matches(path: &std::path::Path, expected: &str) -> Result<bool> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()) == expected)
}

/// A chunk whose vector has already been produced. Pulling the embedding step
/// out of the storage step lets tests exercise the real SQL/vec0 wiring with
/// hand-crafted vectors, without loading the model.
pub(crate) struct PreparedChunk {
    pub input: ChunkInput,
    pub vector: Vec<f32>,
}

/// Managed plugin state: the shared app DB, the lazily-loaded embedder, and the
/// model directory to load it from.
pub struct EmbeddingSearchRuntime {
    db: Arc<Db>,
    /// Lazy-loaded EmbeddingGemma session. `None` until the first embed call.
    ///
    /// TODO(WS-B1): idle-unload — drop the session back to `None` after a period
    /// of inactivity to release ~300 MB. Deliberately not a timer yet.
    embedder: Mutex<Option<TextEmbedder>>,
    model_dir: PathBuf,
    schema_ready: OnceCell<()>,
}

impl EmbeddingSearchRuntime {
    pub fn new(db: Arc<Db>, model_dir: PathBuf) -> Self {
        Self {
            db,
            embedder: Mutex::new(None),
            model_dir,
            schema_ready: OnceCell::new(),
        }
    }

    fn pool(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    async fn ensure_schema(&self) -> Result<()> {
        self.schema_ready
            .get_or_try_init(|| async { hypr_db_app::prepare_schema(self.db.as_ref()).await })
            .await?;
        Ok(())
    }

    /// Whether the model artifacts required for a real embed are present on disk.
    fn model_downloaded(&self) -> bool {
        self.model_dir.join(MODEL_FILE).is_file() && self.model_dir.join(TOKENIZER_FILE).is_file()
    }

    /// Download-on-first-run: fetch every artifact in the pinned manifest into
    /// `model_dir`, streaming to a `.part` file while SHA-256-hashing, verifying
    /// against the pinned digest before the atomic rename. Idempotent: an
    /// artifact already present with the right hash is skipped. `on_progress`
    /// receives per-artifact byte counts so the UI can render a bar. On any
    /// failure the partial `.part` is left for a retry to overwrite; no
    /// half-written final file is ever exposed.
    pub async fn download_model<F: Fn(DownloadProgress)>(&self, on_progress: F) -> Result<()> {
        std::fs::create_dir_all(&self.model_dir)?;
        let total_bytes: u64 = hypr_text_embedding::ARTIFACTS.iter().map(|a| a.size).sum();
        let mut done_bytes: u64 = 0;
        let client = reqwest::Client::new();

        for artifact in hypr_text_embedding::ARTIFACTS {
            let final_path = self.model_dir.join(artifact.name);
            if final_path.is_file() && sha256_file_matches(&final_path, artifact.sha256)? {
                done_bytes += artifact.size;
                on_progress(DownloadProgress::new(
                    artifact.name,
                    artifact.size,
                    artifact.size,
                    done_bytes,
                    total_bytes,
                ));
                continue;
            }

            let part_path = self.model_dir.join(format!("{}.part", artifact.name));
            let mut resp = client
                .get(artifact.url)
                .send()
                .await
                .map_err(|e| Error::Download(e.to_string()))?
                .error_for_status()
                .map_err(|e| Error::Download(e.to_string()))?;

            let mut file = tokio::fs::File::create(&part_path).await?;
            let mut hasher = Sha256::new();
            let mut written: u64 = 0;
            use tokio::io::AsyncWriteExt as _;
            while let Some(chunk) = resp
                .chunk()
                .await
                .map_err(|e| Error::Download(e.to_string()))?
            {
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
                written += chunk.len() as u64;
                on_progress(DownloadProgress::new(
                    artifact.name,
                    written,
                    artifact.size,
                    done_bytes + written,
                    total_bytes,
                ));
            }
            let _ = &mut resp;
            file.flush().await?;
            drop(file);

            let actual = hex_lower(&hasher.finalize());
            if actual != artifact.sha256 {
                let _ = std::fs::remove_file(&part_path);
                return Err(Error::Integrity {
                    name: artifact.name.to_string(),
                    expected: artifact.sha256.to_string(),
                    actual,
                });
            }
            std::fs::rename(&part_path, &final_path)?;
            done_bytes += artifact.size;
        }
        Ok(())
    }

    /// Load the embedder into the mutex on first use, then reuse it. The load is
    /// CPU/IO-blocking (~300 MB), so it runs on the blocking pool.
    async fn ensure_embedder(&self) -> Result<()> {
        let mut guard = self.embedder.lock().await;
        if guard.is_none() {
            let model_dir = self.model_dir.clone();
            let embedder = tokio::task::spawn_blocking(move || TextEmbedder::load(&model_dir))
                .await
                .map_err(|e| Error::Join(e.to_string()))??;
            *guard = Some(embedder);
        }
        Ok(())
    }

    async fn embed_docs(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.ensure_embedder().await?;
        let mut guard = self.embedder.lock().await;
        let embedder = guard.as_mut().expect("ensure_embedder populated the slot");
        Ok(embedder.embed_docs(texts)?)
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.ensure_embedder().await?;
        let mut guard = self.embedder.lock().await;
        let embedder = guard.as_mut().expect("ensure_embedder populated the slot");
        Ok(embedder.embed_query(query)?)
    }

    // ---- Public command surface -------------------------------------------

    pub async fn embed_and_index_chunks(
        &self,
        session_id: String,
        chunks: Vec<ChunkInput>,
    ) -> Result<u32> {
        self.ensure_schema().await?;
        if chunks.is_empty() {
            return Ok(0);
        }

        // Idempotent re-index: skip any chunk already stored under the same
        // deterministic chunk_id (which encodes its content_hash).
        let mut fresh: Vec<ChunkInput> = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let chunk_id = compute_chunk_id(&session_id, &chunk.source_type, &chunk.content_hash);
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM embedding_chunks WHERE chunk_id = ?")
                    .bind(&chunk_id)
                    .fetch_optional(self.pool())
                    .await?;
            if exists.is_none() {
                fresh.push(chunk);
            }
        }
        if fresh.is_empty() {
            return Ok(0);
        }

        let texts: Vec<&str> = fresh.iter().map(|c| c.text.as_str()).collect();
        let vectors = self.embed_docs(&texts).await?;

        let prepared: Vec<PreparedChunk> = fresh
            .into_iter()
            .zip(vectors)
            .map(|(input, vector)| PreparedChunk { input, vector })
            .collect();

        self.store_chunk_vectors(&session_id, prepared).await
    }

    /// Insert already-embedded chunks across the three tables in one
    /// transaction. Returns the number of newly indexed chunks (duplicates are
    /// skipped). Test-visible so the storage/query layer can be exercised
    /// without the model.
    pub(crate) async fn store_chunk_vectors(
        &self,
        session_id: &str,
        prepared: Vec<PreparedChunk>,
    ) -> Result<u32> {
        self.ensure_schema().await?;
        if prepared.is_empty() {
            return Ok(0);
        }

        let now = current_millis();
        let mut tx = self.pool().begin().await?;
        let mut inserted = 0u32;

        for PreparedChunk { input, vector } in &prepared {
            let chunk_id = compute_chunk_id(session_id, &input.source_type, &input.content_hash);

            let exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM embedding_chunks WHERE chunk_id = ?")
                    .bind(&chunk_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if exists.is_some() {
                continue;
            }

            // Simplest correct rowid allocation: monotonic MAX+1. Uncommitted
            // inserts earlier in this same transaction are visible on this
            // connection, so a batch stays collision-free.
            let rowid: i64 =
                sqlx::query_scalar("SELECT COALESCE(MAX(rowid), 0) + 1 FROM embedding_vector_map")
                    .fetch_one(&mut *tx)
                    .await?;

            sqlx::query(
                "INSERT INTO embedding_chunks \
                 (chunk_id, session_id, source_type, content_hash, text, start_ms, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&chunk_id)
            .bind(session_id)
            .bind(&input.source_type)
            .bind(&input.content_hash)
            .bind(&input.text)
            .bind(input.start_ms)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            let bytes = vector_to_le_bytes(vector);
            sqlx::query("INSERT INTO embedding_vectors (rowid, embedding) VALUES (?, ?)")
                .bind(rowid)
                .bind(bytes)
                .execute(&mut *tx)
                .await?;

            sqlx::query("INSERT INTO embedding_vector_map (rowid, chunk_id) VALUES (?, ?)")
                .bind(rowid)
                .bind(&chunk_id)
                .execute(&mut *tx)
                .await?;

            inserted += 1;
        }

        tx.commit().await?;
        Ok(inserted)
    }

    pub async fn delete_session_chunks(&self, session_id: String) -> Result<u32> {
        self.ensure_schema().await?;

        let mut tx = self.pool().begin().await?;

        // vec0 virtual tables are not reached by FK cascade, so their rowids
        // must be deleted explicitly. Look them up before the chunks vanish.
        let rowids: Vec<i64> = sqlx::query_scalar(
            "SELECT m.rowid FROM embedding_vector_map m \
             JOIN embedding_chunks c ON c.chunk_id = m.chunk_id \
             WHERE c.session_id = ?",
        )
        .bind(&session_id)
        .fetch_all(&mut *tx)
        .await?;

        for rowid in &rowids {
            sqlx::query("DELETE FROM embedding_vectors WHERE rowid = ?")
                .bind(rowid)
                .execute(&mut *tx)
                .await?;
        }

        // FK cascade removes the matching embedding_vector_map rows.
        let deleted = sqlx::query("DELETE FROM embedding_chunks WHERE session_id = ?")
            .bind(&session_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        tx.commit().await?;
        Ok(deleted as u32)
    }

    pub async fn semantic_search(
        &self,
        query: String,
        k: u32,
        session_id: Option<String>,
    ) -> Result<Vec<SearchHit>> {
        self.ensure_schema().await?;
        // Empty query: return nothing rather than surfacing an embed error.
        if query.trim().is_empty() || k == 0 {
            return Ok(Vec::new());
        }
        let vector = self.embed_query(&query).await?;
        self.knn_join(&vector, k, session_id.as_deref()).await
    }

    /// KNN the vec0 table for the top-k nearest rowids, then join back to chunk
    /// metadata (applying the optional session filter) and order by distance.
    /// Test-visible so the vector path is exercisable without the model.
    pub(crate) async fn knn_join(
        &self,
        vector: &[f32],
        k: u32,
        session_id: Option<&str>,
    ) -> Result<Vec<SearchHit>> {
        self.ensure_schema().await?;
        if k == 0 {
            return Ok(Vec::new());
        }

        let query_bytes = vector_to_le_bytes(vector);
        let knn: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT rowid, distance FROM embedding_vectors \
             WHERE embedding MATCH ? ORDER BY distance LIMIT ?",
        )
        .bind(query_bytes)
        .bind(k as i64)
        .fetch_all(self.pool())
        .await?;

        if knn.is_empty() {
            return Ok(Vec::new());
        }

        let distances: HashMap<i64, f64> = knn.iter().copied().collect();
        let rowids: Vec<i64> = knn.iter().map(|(rowid, _)| *rowid).collect();

        let placeholders = vec!["?"; rowids.len()].join(",");
        let mut sql = format!(
            "SELECT m.rowid, c.chunk_id, c.session_id, c.source_type, c.text, c.start_ms \
             FROM embedding_vector_map m \
             JOIN embedding_chunks c ON c.chunk_id = m.chunk_id \
             WHERE m.rowid IN ({placeholders})"
        );
        if session_id.is_some() {
            sql.push_str(" AND c.session_id = ?");
        }

        // `sql` is built only from a fixed template plus a `?`-placeholder list
        // whose count equals `rowids.len()`; no user data is interpolated.
        let mut q = sqlx::query_as::<_, (i64, String, String, String, String, Option<i64>)>(
            sqlx::AssertSqlSafe(sql),
        );
        for rowid in &rowids {
            q = q.bind(rowid);
        }
        if let Some(sid) = session_id {
            q = q.bind(sid);
        }
        let rows = q.fetch_all(self.pool()).await?;

        let mut hits: Vec<SearchHit> = rows
            .into_iter()
            .map(
                |(rowid, chunk_id, session_id, source_type, text, start_ms)| SearchHit {
                    chunk_id,
                    session_id,
                    source_type,
                    text,
                    start_ms,
                    distance: distances.get(&rowid).copied().unwrap_or(f64::MAX) as f32,
                },
            )
            .collect();

        hits.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(hits)
    }

    pub async fn index_status(&self) -> Result<IndexStatus> {
        self.ensure_schema().await?;
        let chunk_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM embedding_chunks")
            .fetch_one(self.pool())
            .await?;
        Ok(IndexStatus {
            model_downloaded: self.model_downloaded(),
            chunk_count,
        })
    }
}

/// Deterministic per-content chunk id: `sha256(session_id | source_type |
/// content_hash)`. Stable across re-index (idempotency) and changes whenever the
/// content changes (a new content_hash yields a new id).
fn compute_chunk_id(session_id: &str, source_type: &str, content_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(source_type.as_bytes());
    hasher.update([0u8]);
    hasher.update(content_hash.as_bytes());
    let digest = hasher.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn vector_to_le_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn current_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
