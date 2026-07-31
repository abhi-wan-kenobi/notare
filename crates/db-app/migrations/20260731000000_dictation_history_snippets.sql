-- Promote dictation_history to a first-class, searchable history (v0.5.1
-- "Snippets"): keep the pre-cleanup raw transcript, tag source/model/duration,
-- allow pinning, and recover discarded dictations. Forward-only: existing 0.5.0
-- rows keep their cleaned `text` and take the column defaults.
--
-- FTS5 is compiled into the bundled SQLite (libsqlite3-sys `bundled` builds
-- with -DSQLITE_ENABLE_FTS5), so the virtual table + triggers below are safe to
-- create at startup.

ALTER TABLE dictation_history ADD COLUMN raw_text TEXT;
ALTER TABLE dictation_history ADD COLUMN source TEXT NOT NULL DEFAULT 'dictation';
ALTER TABLE dictation_history ADD COLUMN model TEXT;
ALTER TABLE dictation_history ADD COLUMN duration_ms INTEGER;
ALTER TABLE dictation_history ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
-- Reserved for future in-app playback; unused for now.
ALTER TABLE dictation_history ADD COLUMN audio_path TEXT;
ALTER TABLE dictation_history ADD COLUMN status TEXT NOT NULL DEFAULT 'delivered';

-- Pruning keeps the newest unpinned rows and lists filter/paginate by recency.
CREATE INDEX IF NOT EXISTS idx_dictation_history_pinned_created_at
  ON dictation_history (pinned, created_at);

-- Full-text index over the cleaned text + raw transcript. A self-contained
-- (non external-content) fts5 table carrying the base table's TEXT id as an
-- UNINDEXED column, so search joins `fts.id = dictation_history.id`.
-- Deliberately NOT keyed on rowid: dictation_history's TEXT PRIMARY KEY means
-- its rowid is implicit, and implicit rowids can be renumbered by VACUUM -
-- which would silently corrupt a rowid-coupled FTS mapping.
CREATE VIRTUAL TABLE IF NOT EXISTS dictation_history_fts USING fts5(
  text,
  raw_text,
  id UNINDEXED
);

-- Backfill existing rows (raw_text is NULL for pre-migration entries). The
-- NOT EXISTS guard makes a replay (manual re-run against a tinkered DB) a
-- no-op instead of doubling every FTS row; the runner itself never replays.
INSERT INTO dictation_history_fts (text, raw_text, id)
  SELECT h.text, h.raw_text, h.id FROM dictation_history h
  WHERE NOT EXISTS (
    SELECT 1 FROM dictation_history_fts f WHERE f.id = h.id
  );

CREATE TRIGGER IF NOT EXISTS dictation_history_fts_ai
AFTER INSERT ON dictation_history BEGIN
  INSERT INTO dictation_history_fts (text, raw_text, id)
    VALUES (new.text, new.raw_text, new.id);
END;

CREATE TRIGGER IF NOT EXISTS dictation_history_fts_ad
AFTER DELETE ON dictation_history BEGIN
  DELETE FROM dictation_history_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS dictation_history_fts_au
AFTER UPDATE ON dictation_history BEGIN
  DELETE FROM dictation_history_fts WHERE id = old.id;
  INSERT INTO dictation_history_fts (text, raw_text, id)
    VALUES (new.text, new.raw_text, new.id);
END;
