-- Dictation history ("in-app clipboard"): every completed dictation is kept
-- (capped at 50, pruned on insert by the frontend) so the text can be
-- re-copied from Settings -> Dictation after the fact.
CREATE TABLE IF NOT EXISTS dictation_history (
  id TEXT PRIMARY KEY NOT NULL,
  text TEXT NOT NULL,
  -- Output mode of the session that produced the text: 'type' | 'batch'.
  mode TEXT NOT NULL DEFAULT 'type',
  -- Whether cleanup (basic or LLM) was applied to `text` (0 = raw).
  cleaned INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_dictation_history_created_at
  ON dictation_history (created_at);
