CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(title, summary, memo, content=sessions, content_rowid=rowid);
CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
  INSERT INTO sessions_fts(rowid, title, summary, memo) VALUES (new.rowid, new.title, new.summary, new.memo);
END;
CREATE TRIGGER IF NOT EXISTS sessions_ad AFTER DELETE ON sessions BEGIN
  INSERT INTO sessions_fts(sessions_fts, rowid, title, summary, memo) VALUES ('delete', old.rowid, old.title, old.summary, old.memo);
END;
CREATE TRIGGER IF NOT EXISTS sessions_au AFTER UPDATE ON sessions BEGIN
  INSERT INTO sessions_fts(sessions_fts, rowid, title, summary, memo) VALUES ('delete', old.rowid, old.title, old.summary, old.memo);
  INSERT INTO sessions_fts(rowid, title, summary, memo) VALUES (new.rowid, new.title, new.summary, new.memo);
END;
