# `plugins/db`

## TypeScript Usage

- Use `execute(sql, params?)` for one-shot reads or writes.
- Use `subscribe(sql, params, { onData, onError })` for live query updates.
- Build React hooks such as `useLiveQuery` in app code on top of `subscribe`; do not ship them from this plugin.

## Boundaries

- Keep schema creation, migrations, and DB initialization in Rust.
- This plugin should only expose query execution and live subscription primitives to TypeScript.
- Queries should target the app database managed at `app_data_dir()/app.db`.
