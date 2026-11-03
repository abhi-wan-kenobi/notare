---
name: sqlite-schema-design
description: Design or review schemas for cr-sqlite CRRs using conflict-free replicated relation constraints, not generic SQLite advice. Use when adding synced tables, changing synced columns, or planning CRR-safe migrations.
metadata:
  internal: true
---

## Goal

Design tables and migrations that behave correctly under cr-sqlite's Conflict-free Replicated Relation (CRR) replication model.

This skill is specifically for cr-sqlite CRR-backed schemas in Notare:

- Synced-table policy is centralized in `crates/db-app` (`SYNCED_TABLES`, `ELIGIBLE_PENDING_PROOF`, `NO_GO_TABLES`).
- Tables become CRRs via `crsql_as_crr` only when the user turns sync on (`plugins/db/src/change_source.rs::enable_crr_tables`).
- Primary keys must be application-generated `TEXT PRIMARY KEY NOT NULL` (UUIDv4) or deterministically derived.
- DDL altering a synced table must use db-migrate's `MigrationScope::CrrAlter { table_name }` (`crsql_begin_alter` / `crsql_commit_alter`).
- Secondary unique indices are rejected by cr-sqlite; all required non-PK columns must have explicit `DEFAULT`s.

Do not treat this as ordinary SQLite schema design. cr-sqlite imposes strict rules around primary keys, column defaults, unique indices, and schema evolution windows.

---

## Workflow

### 1. Classify The Table According To App Policy

Before proposing DDL, classify the table according to `crates/db-app` policy (`crates/db-app/src/sync_tables.rs`):

1. **`SYNCED_TABLES`**: Actively converted to CRRs when sync is turned on.
   - Current tables: `sessions`, `session_documents`, `transcripts`, `action_items`, `tags`, `session_tags`.
   - **Rule**: Every table added here must have a proven convergence proof verifying merge behavior under concurrent edits and tombstones.
2. **`ELIGIBLE_PENDING_PROOF`**: Structurally eligible for sync but not yet enabled.
   - Current tables: `organizations`, `templates`, `chat_groups`, `chat_messages`.
   - Never add to `SYNCED_TABLES` before a convergence proof is established.
3. **`NO_GO_TABLES`**: Proven unsafe to sync due to duplicate entity creation under concurrency.
   - Current tables: `humans`, `session_participants`, `calendars`, `events`.
   - Never add to `SYNCED_TABLES` without deterministic primary key backfills.
4. **Local-Only / Ephemeral Tables**:
   - Ledger tables, vector index tables (`embedding_vector_map`), caches, and cursor tracking (`sync_peer_cursors`) remain non-CRRs.

---

### 2. Primary Key Strategy: App-Generated UUIDs

For synced tables:

- Always declare:
  ```sql
  id TEXT PRIMARY KEY NOT NULL
  ```
- **Explicit `NOT NULL` is required**: In SQLite `STRICT` mode, cr-sqlite rejects primary keys without an explicit `NOT NULL` constraint.
- **Do not use autoincrementing integers**: Autoincrement integers collide across devices creating rows offline.
- **Mint IDs in application code**: Generate IDs via `uuid::Uuid::new_v4().to_string()` in Rust or `crypto.randomUUID()` in TypeScript.
- **Do not rely on SQL helper functions**: Do not use custom SQL UUID functions. Mint IDs before inserting rows.

---

### 3. Deterministic IDs for Find-Or-Create Rows

A critical CRDT defect occurs when entities are created behind a local `NOT EXISTS (...)` check using random UUIDs:

- Two offline devices independently create the same logical entity (e.g. adding a person with the same email, or a calendar poller inserting an event with the same external tracking ID).
- Both devices pass their local `NOT EXISTS` check and mint different random UUIDs.
- After replication, both rows exist forever. No CRDT merge engine can unify two distinct primary keys.

**Required Solution**: Derive primary keys deterministically from the entity's domain identity using domain-separated hashes (`crates/db-app/src/id_backfill.rs`):

```text
humans.id               = "h_"  + hex(sha256("notare:human:v1:"       + norm_email))[0..32]
organizations.id        = "o_"  + hex(sha256("notare:org:v1:"         + norm_name))[0..32]
session_participants.id = "sp_" + hex(sha256("notare:participant:v1:" + session_id + "\n" + member_key))[0..32]
```

Where:
- `norm_email = lower(trim(email))` after Unicode NFC normalization.
- `norm_name = lower(trim(name))`.
- If an entity cannot be identified deterministically (e.g. `calendars` and `events` where external provider IDs live in non-PK columns), it is classified as `NO_GO_TABLES` until the schema is refactored.

---

### 4. Make Inserts Merge-Safe With Explicit Defaults

cr-sqlite replicates and merges column-by-column through `crsql_changes`. When a row insert merges on a remote node, missing or omitted columns receive their schema default values:

- **Every non-primary-key `NOT NULL` column must have an explicit `DEFAULT`**.
- cr-sqlite's `crsql_as_crr` explicitly rejects tables with a `NOT NULL` column lacking a `DEFAULT` (e.g. `voice_profiles` is rejected by the engine with `"NOT NULL column without a DEFAULT"`).
- Prefer simple scalar defaults over nullable columns when the field is logically always present.

**Good**:
```sql
title TEXT NOT NULL DEFAULT '',
archived INTEGER NOT NULL DEFAULT 0,
created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
```

**Bad**:
```sql
title TEXT NOT NULL,
archived INTEGER NOT NULL
```

---

### 5. No Secondary Unique Constraints

- **cr-sqlite rejects tables with secondary unique indices**: The engine fails `crsql_as_crr` if any `UNIQUE` index or constraint exists besides the primary key (e.g. `embedding_vector_map` is rejected with `"unique indices"`).
- Uniqueness across multiple columns must be enforced through application logic or by making the unique tuple part of a deterministic primary key.

---

### 6. Be Conservative With Foreign Keys

- CRDT changes replicate and apply out-of-order and asynchronously across devices.
- Cascading deletes (`ON DELETE CASCADE`) interact unpredictably with CRDT tombstones.
- If foreign keys are present:
  - Avoid cascading delete actions on synced tables.
  - Make foreign key columns nullable if child records can arrive before parents.
  - Rely on application-level soft deletes (`deleted_at TEXT`) or cr-sqlite row tombstones (`cid = "-1"`).

---

### 7. Execute DDL Inside `MigrationScope::CrrAlter`

Altering the schema of a table that has already been converted to a CRR requires executing the DDL within a cr-sqlite alter window:

```sql
SELECT crsql_begin_alter('table_name');
-- execute ALTER TABLE statements
SELECT crsql_commit_alter('table_name');
```

In Notare's migration system (`crates/db-migrate` & `crates/db-app`):
- Declare the migration step with `MigrationScope::CrrAlter { table_name }`:
  ```rust
  hypr_db_migrate::MigrationStep {
      id: "20260723130000_action_items_v2",
      scope: hypr_db_migrate::MigrationScope::CrrAlter {
          table_name: "action_items",
      },
      sql: include_str!("../migrations/20260723130000_action_items_v2.sql"),
  }
  ```
- `db-migrate` ensures connection affinity: `crsql_begin_alter`, the DDL, and `crsql_commit_alter` execute sequentially on the same checked-out connection handle.
- If the table is not currently a CRR (e.g. on a device where sync was never enabled), `db-migrate` runs the DDL as normal SQLite execution without invoking the alter window.

---

### 8. Respect STRICT Mode

Notare schemas enforce SQLite `STRICT` mode. Under cr-sqlite:
- Primary key must be `id TEXT PRIMARY KEY NOT NULL`.
- All column types must be valid STRICT types (`INTEGER`, `REAL`, `TEXT`, `BLOB`, `ANY`).
- cr-sqlite's value codec (`SqlValue`) decodes SQLite runtime storage classes directly, preserving numeric and binary precision across nodes.

---

## Design Defaults For Synced Tables

Unless explicitly approved otherwise:

```sql
CREATE TABLE example_entity (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    deleted_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
```

Checklist:
- `id TEXT PRIMARY KEY NOT NULL` (app-generated UUIDv4 or deterministic hash).
- `STRICT` mode enabled.
- Every non-PK `NOT NULL` column carries an explicit `DEFAULT`.
- No secondary `UNIQUE` indices or constraints.
- No `ON DELETE CASCADE` foreign keys.
- Migration declared with `MigrationScope::CrrAlter { table_name }`.
- Table registered in `crates/db-app/src/sync_tables.rs`.
