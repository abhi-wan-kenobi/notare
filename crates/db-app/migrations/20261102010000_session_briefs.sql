CREATE TABLE IF NOT EXISTS session_briefs (
  id           TEXT PRIMARY KEY NOT NULL,
  session_id   TEXT NOT NULL DEFAULT '',
  event_id     TEXT NOT NULL DEFAULT '',
  generated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  content_md   TEXT NOT NULL DEFAULT '',
  model        TEXT NOT NULL DEFAULT '',
  dismissed_at TEXT,
  created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_session_briefs_session_id ON session_briefs (session_id);
CREATE INDEX IF NOT EXISTS idx_session_briefs_event_id ON session_briefs (event_id);
CREATE INDEX IF NOT EXISTS idx_session_briefs_dismissed_at ON session_briefs (dismissed_at);
