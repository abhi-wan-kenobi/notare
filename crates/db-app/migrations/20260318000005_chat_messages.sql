CREATE TABLE IF NOT EXISTS chat_messages (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL DEFAULT '' REFERENCES sessions(id),
  role TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  user_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);
