CREATE TABLE IF NOT EXISTS words (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL DEFAULT '' REFERENCES sessions(id),
  text TEXT NOT NULL DEFAULT '',
  start_ms INTEGER NOT NULL DEFAULT 0,
  end_ms INTEGER NOT NULL DEFAULT 0,
  channel INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL DEFAULT 'final'
);
CREATE INDEX IF NOT EXISTS idx_words_session ON words(session_id, start_ms);
