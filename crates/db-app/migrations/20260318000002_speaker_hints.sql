CREATE TABLE IF NOT EXISTS speaker_hints (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL DEFAULT '' REFERENCES sessions(id),
  word_id TEXT NOT NULL DEFAULT '' REFERENCES words(id),
  kind TEXT NOT NULL DEFAULT '',
  speaker_index INTEGER,
  provider TEXT,
  channel INTEGER,
  human_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_hints_session ON speaker_hints(session_id);
CREATE INDEX IF NOT EXISTS idx_hints_word ON speaker_hints(word_id);
