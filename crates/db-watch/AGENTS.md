# `db-watch`

## Role

- `db-watch` is pure dependency-tracking infra for SQL watches.
- It maps `query -> referenced tables` and `changed tables -> affected watch ids`.
- It does not own a database, a pool, async tasks, channels, Tauri state, or JS interfaces.

## Invariants

- Keep this crate `unsafe`-free.
- Keep this crate transport-agnostic. No Tauri, no app runtime, no plugin types.
- `extract_tables(...)` is the single table-dependency extraction path for live-query targeting.
- `TableDeps` is table-granular only. It does not attempt row-level or predicate-level invalidation.
- Empty table sets are allowed and should behave as “never matches”.
- `affected(...)` should remain deduped even if the same table name appears multiple times in the input.

## Design Notes

- If something requires SQLite connection hooks, pooled connection setup, or cross-task fan-out, it belongs below this crate, usually in `db-core2`.
- If something requires query reruns, subscription state, channels, or UI notifications, it belongs above this crate, usually in a plugin/runtime layer.
- Prefer adding tests here when expanding SQL parsing or watch-targeting behavior; this crate is the safe place to prove invalidation semantics without app/runtime coupling.
