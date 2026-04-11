CREATE TABLE IF NOT EXISTS daily_notes (
  id          TEXT PRIMARY KEY NOT NULL,
  date        TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '{}',
  user_id     TEXT NOT NULL DEFAULT '',
  visibility  TEXT NOT NULL DEFAULT 'public',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_notes_date_user
ON daily_notes(date, user_id);

CREATE TABLE IF NOT EXISTS daily_summaries (
  id                 TEXT PRIMARY KEY NOT NULL,
  daily_note_id      TEXT NOT NULL DEFAULT '',
  date               TEXT NOT NULL DEFAULT '',
  content            TEXT NOT NULL DEFAULT '',
  timeline_json      TEXT NOT NULL DEFAULT '[]',
  topics_json        TEXT NOT NULL DEFAULT '[]',
  status             TEXT NOT NULL DEFAULT 'idle',
  source_cursor_ms   INTEGER NOT NULL DEFAULT 0,
  source_fingerprint TEXT NOT NULL DEFAULT '',
  generation_error   TEXT NOT NULL DEFAULT '',
  generated_at       TEXT NOT NULL DEFAULT '',
  created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_summaries_daily_note
ON daily_summaries(daily_note_id);

CREATE INDEX IF NOT EXISTS idx_daily_summaries_date
ON daily_summaries(date);
