CREATE TABLE IF NOT EXISTS recipes (
  id         TEXT PRIMARY KEY NOT NULL,
  title      TEXT NOT NULL,
  prompt     TEXT NOT NULL,
  scope      TEXT NOT NULL DEFAULT 'single',
  builtin    INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_recipes_scope ON recipes (scope);
CREATE INDEX IF NOT EXISTS idx_recipes_builtin ON recipes (builtin);
