-- Per-peer pull cursor for P2P sync (protocol v2): the last (db_version, seq)
-- position in a remote peer's crsql_changes stream that this device applied.
-- Device-local bookkeeping: never a CRR, never in SYNCED_TABLES.
CREATE TABLE IF NOT EXISTS sync_peer_cursors (
  peer_id    TEXT PRIMARY KEY NOT NULL,
  db_version INTEGER NOT NULL,
  seq        INTEGER NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;
