-- On-device semantic search index (WS-B1). Two tables:
--   embedding_chunks  — canonical chunk metadata (STRICT, cloudsync-safe).
--   embedding_vectors — sqlite-vec `vec0` virtual table holding the 512-d
--                       Matryoshka-truncated EmbeddingGemma vectors.
--
-- The `vec0` module is registered as a SQLite auto-extension in
-- db-core::apply_internal_connect_policy before any pool opens, so the virtual
-- table below is creatable during migration on every db-core connection.

CREATE TABLE IF NOT EXISTS embedding_chunks (
    chunk_id     TEXT    NOT NULL PRIMARY KEY,
    session_id   TEXT    NOT NULL,
    -- 'note' | 'transcript' — which surface this chunk came from.
    source_type  TEXT    NOT NULL,
    -- Hash of the source text at index time; lets the indexer skip re-embedding
    -- unchanged content and detect staleness.
    content_hash TEXT    NOT NULL,
    text         TEXT    NOT NULL,
    -- Word-timing anchor for transcript chunks (ms into the recording); NULL
    -- for note chunks. Used for jump-to-source via seekAndPlay.
    start_ms     INTEGER,
    created_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_embedding_chunks_session
    ON embedding_chunks (session_id);

CREATE INDEX IF NOT EXISTS idx_embedding_chunks_content_hash
    ON embedding_chunks (content_hash);

-- Dense vector store. rowid links back to embedding_chunks via the mapping
-- table below (vec0 virtual tables cannot hold arbitrary FK columns).
CREATE VIRTUAL TABLE IF NOT EXISTS embedding_vectors USING vec0(
    embedding float[512]
);

-- rowid (embedding_vectors) <-> chunk_id (embedding_chunks). Kept separate so
-- deleting a session's chunks is a plain DELETE + a vec0 rowid delete.
CREATE TABLE IF NOT EXISTS embedding_vector_map (
    rowid    INTEGER NOT NULL PRIMARY KEY,
    chunk_id TEXT    NOT NULL UNIQUE,
    FOREIGN KEY (chunk_id) REFERENCES embedding_chunks (chunk_id) ON DELETE CASCADE
) STRICT;
