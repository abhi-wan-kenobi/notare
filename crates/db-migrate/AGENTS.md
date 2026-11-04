# `db-migrate`

## Role

App-database migration execution. Narrow port of `sqlx` migration behavior with one intentional divergence: explicit per-step scope (`Plain` vs `CrrAlter`) to enforce connection affinity during DDL on cr-sqlite CRRs.

## Owns

- Migration orchestration and `_sqlx_migrations` bookkeeping.
- `MigrationStep` → `sqlx::migrate::Migration` translation.
- Validation of step ids, duplicate versions, `CrrAlter`-target eligibility (`DbSchema::validate_crr_table`).
- Execution semantics for `Plain` vs `CrrAlter { table_name }`.

## Does Not Own

- Pool creation or database opening.
- cr-sqlite extension loading or CRR conversion (`db-core`, `plugins/db`).
- App table definitions, row types, query APIs, migration SQL contents.
- Inference of whether a step "looks like" a CRR alter.

## Invariants

- `CrrAlter` runs `crsql_begin_alter` → DDL → `crsql_commit_alter` on the same checked-out connection (`Db::begin_crr_alter_on` / `CrrAlter::commit`). Pool-level execution is not acceptable.
- Every alter window closes its connection (and, on commit, every idle pooled one): cr-sqlite v0.16.3 never refreshes a connection's cached CRR columns after an alter. The runner acquires a fresh connection for the next step. A failed DDL or commit rolls back with the closed connection, so the step stays unapplied.
- When cr-sqlite is not loaded or the table is not a CRR yet, `CrrAlter` falls back to normal SQLite execution; it never converts a table.
- Preserves `sqlx` semantics: ordered apply, checksum validation, dirty-version detection, idempotent re-run.
- Migration scope is explicit in the manifest; never inferred from SQL text.
