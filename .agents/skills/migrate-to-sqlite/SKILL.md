---
name: migrate-to-sqlite
description: Migrate a TinyBase table to SQLite. Use when asked to move a data domain (e.g. templates, chat shortcuts, vocabs) from the TinyBase store to the app SQLite database.
---

## Architecture

- **Schema source of truth:** Rust migration in `crates/db-app/migrations/`
- **Drizzle mirror:** `packages/db/src/schema.ts` (typed TS query interface, not schema management)
- **Reads (reactive):** `useDrizzleLiveQuery` — calls `.toSQL()` on a Drizzle query, feeds `{sql, params}` to the underlying `useLiveQuery` which uses `subscribe()` from `@hypr/plugin-db`
- **Reads (imperative):** `db.select()...` through the Drizzle sqlite-proxy driver
- **Writes:** `db.insert()`, `db.update()`, `db.delete()` through the Drizzle sqlite-proxy driver, wrapped in `useMutation` from tanstack-query
- **Reactivity loop:** write via `execute` → SQLite change → Rust `db-live-query` notifies subscribers → `useLiveQuery` fires `onData` → React re-renders. No manual invalidation needed.

### Package layers

The DB stack uses a factory/DI pattern across four packages:

1. `@hypr/db-runtime` (`packages/db-runtime/`) — type contracts only: `LiveQueryClient`, `DrizzleProxyClient`, shared row/query types.
2. `@hypr/db` (`packages/db/`) — Drizzle schema (`schema.ts`) + `createDb(client)` factory using `drizzle-orm/sqlite-proxy`. Re-exports Drizzle operators (`eq`, `and`, `sql`, etc.).
3. `@hypr/db-tauri` (`packages/db-tauri/`) — Tauri-specific client that binds `execute`/`executeProxy`/`subscribe` from `@hypr/plugin-db` to the `db-runtime` types.
4. `@hypr/db-react` (`packages/db-react/`) — `createUseLiveQuery(client)` and `createUseDrizzleLiveQuery(client)` factories.

These are wired together in `apps/desktop/src/db/index.ts`, which exports `db`, `useLiveQuery`, and `useDrizzleLiveQuery`. **Consumer code imports from `~/db`, not directly from the packages.**

## Steps

### 1. Rust migration

Add a new timestamped `.sql` file in `crates/db-app/migrations/`. Convention: `YYYYMMDDHHMMSS_name.sql`.

Do NOT include `user_id` columns — it was a TinyBase-era pattern with a hardcoded default. It will be redesigned when multi-device/team support lands.

### 2. Rust ops (optional but recommended)

Add `<domain>_types.rs` and `<domain>_ops.rs` in `crates/db-app/src/` with typed `sqlx::FromRow` structs and CRUD functions. Export from `lib.rs`. These are used by other Rust code and legacy import; the TS side uses Drizzle instead.

### 3. Legacy data import

If the domain had a TinyBase JSON persister file (e.g. `templates.json`), add an import function in `plugins/db/src/migrate.rs` that reads the old file and upserts rows. Call it from `plugins/db/src/runtime.rs` during startup. Guard with an "already imported" check (e.g. table non-empty).

### 4. Drizzle schema

Add the table definition to `packages/db/src/schema.ts` mirroring the migration. Use `{ mode: "json" }` for JSON text columns, `{ mode: "boolean" }` for integer boolean columns. Re-export from `packages/db/src/index.ts` if adding new operator re-exports.

### 5. TS consumer migration

Replace raw TinyBase reads/writes with:
- `useDrizzleLiveQuery(db.select()...)` for reactive reads
- `db.select()...` for imperative reads (returns parsed objects via proxy driver)
- `db.insert()`, `db.update()`, `db.delete()` for writes, wrapped in `useMutation`

Import `db` and `useDrizzleLiveQuery` from `~/db` (the app-level wiring module), and schema tables/operators from `@hypr/db`.

Live query results come from Rust `subscribe` as raw objects (not through the Drizzle driver), so `mapRows` must handle two things:
- **JSON parsing** for JSON text columns (e.g. `sections_json`, `targets_json`).
- **snake_case → camelCase mapping.** Live rows use the raw SQLite column names (`pin_order`, `targets_json`), while Drizzle's `$inferSelect` uses camelCase (`pinOrder`, `targetsJson`). Define a separate `<Domain>LiveRow` type with snake_case keys for `mapRows`, distinct from the Drizzle inferred type. See `TemplateLiveRow` in `apps/desktop/src/templates/queries.ts` for the pattern.

### 6. Remove TinyBase artifacts

- Table definition from `packages/store/src/tinybase.ts`
- Query definitions from `store/tinybase/store/main.ts` (both `QUERIES` object and `_QueryResultRows` type)
- Persister files (e.g. `store/tinybase/persister/<domain>/`)
- Persister registration from `store/tinybase/store/persisters.ts`
- Hooks from `store/tinybase/hooks/` if they existed
- Associated tests and test wrapper setup

### 7. Verify

- `cargo check` and `cargo test -p db-app -p tauri-plugin-db`
- `pnpm -F @hypr/desktop typecheck`
- `pnpm -F @hypr/desktop test`
- `pnpm exec dprint fmt`
