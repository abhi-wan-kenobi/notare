# Upstream source

This directory is a **source-level vendor** of the SQLite Sync extension
(CloudSync), used to build the loadable extension from source with a custom
network layer (see `docs/internal/sync-p2p.md`).

## Provenance

| Field | Value |
|---|---|
| Upstream repo | https://github.com/sqliteai/sqlite-sync |
| Tag | `1.0.12` |
| Commit SHA | `6694c2e8b084d6f33d8bf86742ac1f2b8243bd6e` |
| License | `LICENSE.md` (Elastic License 2.0, "modified for open-source use") |
| Vendor date | 2026-08-26 |
| Cloned from | `/tmp/sqlite-sync-src` (verified `git describe --tags` == `1.0.12`) |

## Contents

- `*.c` / `*.h` at the root — the CloudSync core (block CRDT, payload codec,
  LZ4, private-key/`pk`, utils, `dbutils`).
- `network/` — the default **libcurl** network implementation (`network.c`),
  the public `network.h`, and the custom-network-layer interface
  `network_private.h`.
- `sqlite/` — the SQLite bindings + `sqlite3ext.h` (loadable-extension API
  stubs) + `database_sqlite.c` (SQLite allocator: `dbmem_*` → `sqlite3_*`).
- `modules/fractional-indexing/` — the `sqliteai/fractional-indexing` git
  submodule (MIT, see its own `LICENSE`), compiled into the extension.

## Intentionally NOT vendored (out of scope for the linux from-source build)

- `src/network/network.m` — Apple-native (NSURLSession) implementation; the
  Objective-C reference for a later Apple build (SYNC-9).
- `src/network/cacert.h` — Android-only PEM bundle (pulled in only under
  `__ANDROID__`).
- `src/postgresql/` — PostgreSQL extension backend, compiled only under
  `CLOUDSYNC_POSTGRESQL_BUILD` (never set here).
- Git history / submodule metadata.

## License notice (required by ELv2)

Both license texts are preserved in-tree and must remain with any
redistribution:

- `LICENSE.md` — Elastic License 2.0 (modified for open-source use), with an
  Additional Grant for open-source projects.
- `modules/fractional-indexing/LICENSE` — MIT, `Copyright (c) 2026 SQLite AI`.
