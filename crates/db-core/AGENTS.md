# `db-core`

## Role

Database substrate layer. Owns `Db`/`DbPool`, SQLite open options, pool lifecycle, storage-recreation primitives, and per-connection SQLite wiring. cr-sqlite engine loading (`Db::open_crsqlite`, the `crsqlite` feature) belongs here because it is part of how the database is opened, not how queries are exposed.

## Owns

- Opening local and in-memory SQLite databases.
- SQLite pragmas and connection policy.
- Wiring `db-change::ChangeNotifier` into pool creation via `SqlitePoolOptions::after_connect`.
- Database recreation primitives for upper layers.
- Connection-scoped cr-sqlite helpers (`Db::begin_crr_alter_on` / `CrrAlter`).
- `DbPool` as ergonomic `SqlitePool` wrapper.

## Does Not Own

- App-specific database paths or bootstrap decisions.
- Schema definitions, migrations, migration SQL.
- Hook logic itself (owned by `db-change`).
- Dependency analysis, subscription registries, rerun policy, sink delivery.
- Tauri commands, JS bindings, React hooks.

## Invariants

- Per-connection SQLite setup happens in `SqlitePoolOptions::after_connect`.
- Table-change notifications are best-effort signals for pooled writes only.
- `DbPool` must `Deref`/`AsRef` to `SqlitePool`.
- Schema-agnostic; callers supply migration callbacks.
- cr-sqlite operations requiring executor affinity are wrapped here.
- A db opened with cr-sqlite must be closed through `Db::close`, which finalizes every connection first.
