ALTER TABLE sessions ADD COLUMN event_id TEXT DEFAULT NULL REFERENCES events(id);
CREATE INDEX IF NOT EXISTS idx_sessions_event ON sessions(event_id);
