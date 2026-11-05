CREATE TABLE IF NOT EXISTS session_decisions (
  id          TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL DEFAULT '',
  session_id  TEXT NOT NULL DEFAULT '',
  text        TEXT NOT NULL DEFAULT '',
  owner       TEXT,
  at_ms       INTEGER,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  deleted_at  TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_session_decisions_session_id
  ON session_decisions(session_id);
