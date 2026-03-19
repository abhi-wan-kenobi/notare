CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  title TEXT,
  summary TEXT,
  memo TEXT,
  user_id TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'public',
  folder_id TEXT DEFAULT NULL
);
