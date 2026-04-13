# `db-core2`

## Role

- `db-core2` owns SQLite pool creation and DB lifecycle.
- Connection-level reactive wiring belongs here, including `sqlite3_update_hook`.
- Higher layers should consume table-change events from `Db3`/`DbPool` instead of installing their own SQLite hooks.

## Invariants

- Per-connection SQLite setup must happen in `SqlitePoolOptions::after_connect`.
- Do not install `sqlite3_update_hook` from plugin code or from one borrowed connection; that breaks once the pool has more than one physical connection.
- Table-change notifications are best-effort signals for writes executed through this app’s pooled connections only.
- Change events are table-level, not row-level.
- `DbPool` must continue to deref/as-ref to `SqlitePool` so existing SQL callers stay ergonomic.
- Reactive support should stay additive. Existing open/migrate behavior should not change unless callers opt into consuming change events.

## Design Notes

- If a future change depends on raw SQLite FFI, keep the FFI boundary centralized here.
- If the pool policy changes between single-connection and multi-connection, the hook path should remain correct without upper-layer changes.
- In-memory databases should still be treated carefully; `max_connections(1)` is the safe default there unless shared-memory behavior is explicitly intended.
- Higher layers such as `plugins/db` should handle subscription bookkeeping, coalescing, and rerun policy. `db-core2` should stay focused on “open the DB correctly and emit table mutations”.
