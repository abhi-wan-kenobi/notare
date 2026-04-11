CREATE TABLE IF NOT EXISTS activity_signals (
  id                   TEXT PRIMARY KEY NOT NULL,
  occurred_at_ms       INTEGER NOT NULL DEFAULT 0,
  transition_sequence  INTEGER NOT NULL DEFAULT 0,
  reason               TEXT NOT NULL DEFAULT '',
  app_id               TEXT NOT NULL DEFAULT '',
  bundle_id            TEXT NOT NULL DEFAULT '',
  app_name             TEXT NOT NULL DEFAULT '',
  activity_kind        TEXT NOT NULL DEFAULT '',
  window_title         TEXT NOT NULL DEFAULT '',
  url                  TEXT NOT NULL DEFAULT '',
  domain               TEXT NOT NULL DEFAULT '',
  content_level        TEXT NOT NULL DEFAULT '',
  source               TEXT NOT NULL DEFAULT '',
  text_anchor_identity TEXT NOT NULL DEFAULT '',
  fingerprint          TEXT NOT NULL DEFAULT '',
  payload_json         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
  created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_signals_occurred_at_ms ON activity_signals(occurred_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_signals_sequence ON activity_signals(transition_sequence);
CREATE INDEX IF NOT EXISTS idx_activity_signals_domain ON activity_signals(domain);
CREATE INDEX IF NOT EXISTS idx_activity_signals_app_id ON activity_signals(app_id);
CREATE INDEX IF NOT EXISTS idx_activity_signals_fingerprint ON activity_signals(fingerprint);

CREATE TABLE IF NOT EXISTS activity_screenshots (
  id               TEXT PRIMARY KEY NOT NULL,
  signal_id        TEXT NOT NULL,
  fingerprint      TEXT NOT NULL DEFAULT '',
  captured_at_ms   INTEGER NOT NULL DEFAULT 0,
  image_png        BLOB NOT NULL,
  analysis_summary TEXT,
  analyzed_at_ms   INTEGER,
  created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_screenshots_captured_at ON activity_screenshots(captured_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_screenshots_signal_id ON activity_screenshots(signal_id);
CREATE INDEX IF NOT EXISTS idx_activity_screenshots_fingerprint ON activity_screenshots(fingerprint);
