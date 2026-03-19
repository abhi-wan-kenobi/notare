CREATE TABLE IF NOT EXISTS session_participants (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL DEFAULT '' REFERENCES sessions(id),
  human_id TEXT NOT NULL DEFAULT '' REFERENCES humans(id),
  source TEXT NOT NULL DEFAULT 'manual',
  user_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_sp_session ON session_participants(session_id);
CREATE INDEX IF NOT EXISTS idx_sp_human ON session_participants(human_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sp_session_human ON session_participants(session_id, human_id);
