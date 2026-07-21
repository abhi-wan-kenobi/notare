CREATE TABLE IF NOT EXISTS voice_profiles (
  id            TEXT PRIMARY KEY NOT NULL,
  human_id      TEXT NOT NULL,
  embedding     BLOB NOT NULL,
  dim           INTEGER NOT NULL,
  model         TEXT NOT NULL,
  sample_count  INTEGER NOT NULL DEFAULT 1,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at    TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_voice_profiles_human ON voice_profiles(human_id);
