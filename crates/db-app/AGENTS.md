# `db-app`

## Use This Crate For

- The app-owned SQLite schema shared by desktop and mobile.
- Migration manifest (`APP_MIGRATION_STEPS`) and synced-table policy (`sync_tables.rs`).
- Thin SQL row/upsert types and CRUD helpers for app tables.

## Put Changes Elsewhere When

- Pool opening, pragmas, SQLite hooks, or cr-sqlite engine loading belong in `db-core`; CRR conversion and the sync lifecycle belong in `plugins/db`.
- Migration execution semantics belong in `db-migrate`.
- One-shot transport or live-query transport belongs in `db-execute`, `plugins/db`, or `mobile-bridge`.
- Reactive invalidation logic belongs in `db-reactive`.

## Hard Rules

- Treat migration ids and shipped SQL as append-only. Add a new step instead of mutating an existing one.
- Migration scope is explicit. If DDL touches a table in `SYNCED_TABLES`, declare `MigrationScope::CrrAlter`; never rely on SQL text inspection.
- If a table may ever be synced, make the original DDL CRR-safe: one `TEXT NOT NULL` primary key and defaults on every non-PK `NOT NULL` column (`crsql_as_crr` rejects anything else).
- `SYNC_TABLE_REGISTRY` and `SYNCED_TABLES` are policy, not discovery. Adding or enabling a table here is a deliberate product/runtime decision.
- Desktop and mobile both run `hypr_db_app::schema()`. Schema drift must be fixed here, not in transport layers.
- JSON-shaped fields stay as opaque `TEXT`. Parsing and legacy-shape normalization belong in adapters/importers, not this crate.
- `events.calendar_id` is intentionally not a foreign key today. Adding FK or cascade behavior changes import, delete, and sync semantics; audit all three first.
- Keep helper behavior stable: upserts preserve `created_at`, bump `updated_at`, and list queries stay deterministic unless you intentionally want downstream churn.
- `insert_*_if_missing` is import-only. Normal write paths should use the upsert helpers.
