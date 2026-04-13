CREATE TABLE IF NOT EXISTS activity_observation_events (
  id                   TEXT PRIMARY KEY NOT NULL,
  observation_id       TEXT NOT NULL,
  occurred_at_ms       INTEGER NOT NULL DEFAULT 0,
  event_kind           TEXT NOT NULL DEFAULT '',
  end_reason           TEXT,
  change_class         TEXT,
  app_id               TEXT NOT NULL DEFAULT '',
  bundle_id            TEXT NOT NULL DEFAULT '',
  app_name             TEXT NOT NULL DEFAULT '',
  activity_kind        TEXT NOT NULL DEFAULT '',
  window_title         TEXT NOT NULL DEFAULT '',
  url                  TEXT NOT NULL DEFAULT '',
  domain               TEXT NOT NULL DEFAULT '',
  text_anchor_identity TEXT NOT NULL DEFAULT '',
  observation_key      TEXT NOT NULL DEFAULT '',
  snapshot_json        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(snapshot_json)),
  created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_observation_events_occurred_at_ms
  ON activity_observation_events(occurred_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_observation_events_observation_id
  ON activity_observation_events(observation_id);
CREATE INDEX IF NOT EXISTS idx_activity_observation_events_app_id
  ON activity_observation_events(app_id);

CREATE TABLE IF NOT EXISTS activity_screenshots (
  id               TEXT PRIMARY KEY NOT NULL,
  observation_id   TEXT NOT NULL,
  screenshot_kind  TEXT NOT NULL DEFAULT '',
  scheduled_at_ms  INTEGER NOT NULL DEFAULT 0,
  captured_at_ms   INTEGER NOT NULL DEFAULT 0,
  app_name         TEXT NOT NULL DEFAULT '',
  window_title     TEXT NOT NULL DEFAULT '',
  mime_type        TEXT NOT NULL DEFAULT '',
  width            INTEGER NOT NULL DEFAULT 0,
  height           INTEGER NOT NULL DEFAULT 0,
  sha256           TEXT NOT NULL DEFAULT '',
  image_blob       BLOB NOT NULL,
  snapshot_json    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(snapshot_json)),
  created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_screenshots_captured_at_ms
  ON activity_screenshots(captured_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_screenshots_observation_id
  ON activity_screenshots(observation_id);

CREATE TABLE IF NOT EXISTS activity_observation_analyses (
  id              TEXT PRIMARY KEY NOT NULL,
  observation_id  TEXT NOT NULL,
  screenshot_id   TEXT NOT NULL,
  screenshot_kind TEXT NOT NULL DEFAULT '',
  captured_at_ms  INTEGER NOT NULL DEFAULT 0,
  model_name      TEXT NOT NULL DEFAULT '',
  prompt_version  TEXT NOT NULL DEFAULT '',
  app_name        TEXT NOT NULL DEFAULT '',
  window_title    TEXT NOT NULL DEFAULT '',
  summary         TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_observation_analyses_dedup
  ON activity_observation_analyses(screenshot_id, model_name, prompt_version);
CREATE INDEX IF NOT EXISTS idx_activity_observation_analyses_captured_at_ms
  ON activity_observation_analyses(captured_at_ms);
CREATE INDEX IF NOT EXISTS idx_activity_observation_analyses_observation_id
  ON activity_observation_analyses(observation_id);
