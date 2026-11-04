# P2P Sync — Architecture, Protocol v2, and cr-sqlite Engine Contract

This document is the authoritative engineering reference for Notare's peer-to-peer (P2P) database synchronization architecture.

The sync subsystem replicates SQLite data directly between user devices over encrypted QUIC streams using Conflict-free Replicated Relations (CRRs) powered by **cr-sqlite** (vlcn-io v0.16.3, MIT) and **Protocol v2** implemented in `crates/sync-p2p`.

> **Licensing & Migration Notice (2026-09-02 / 2026-11-04)**:
> In earlier development cycles (v0.6 S0b through SYNC-9), Notare prototyped P2P sync using SQLite Cloud's CloudSync / sqlite-sync extension (Elastic License 2.0) with a custom C network broker (`network_p2p.c`). On 2026-09-02, SQLite Cloud refused a licence for our custom network layer (`docs/LICENSE-NOTE.md`).
> In release 0.6.6 / 0.6.7, the entire CloudSync stack (`crates/cloudsync`, `network_p2p.c`, `from-source` build features, and `cloudsync_from_source_*` CI jobs) was completely removed. Sync migrated to cr-sqlite and protocol v2.
> The historical records of §1–§23 have been moved to `docs/internal/archive/sync-p2p-cloudsync-era.md` and are summarized at the end of this document.

---

## 1. System Architecture Overview

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Desktop Application UI                          │
│        (Settings toggle, device pairing UI, sync status indicators)    │
└────────────────────────────────────┬───────────────────────────────────┘
                                     │
┌────────────────────────────────────▼───────────────────────────────────┐
│              plugins/db (Tauri Plugin & Runtime Lifecycle)             │
│   - plugins/db/src/sync.rs: SyncLifecycle (orchestrates agent & DB)   │
│   - plugins/db/src/change_source.rs: CrsqliteChangeSource             │
│   - build-support/sync_app_gate.rs: Target platform compile gate       │
└──────────────────┬──────────────────────────────────┬──────────────────┘
                   │                                  │
┌──────────────────▼──────────────┐  ┌────────────────▼──────────────────┐
│      crates/sync-p2p (Rust)     │  │        crates/db-core & app       │
│  - protocol.rs (Proto v2 wire)  │  │  - crates/db-core: open_crsqlite  │
│  - session.rs (Bi-stream sync)  │  │  - crates/db-app: SYNCED_TABLES   │
│  - agent.rs (iroh QUIC endpoint)│  │  - crates/db-migrate: CrrAlter    │
│  - crypto.rs (E2E XChaCha20)    │  │  - plugins/db: sync_peer_cursors  │
│  - identity.rs & peers.rs       │  │                                   │
└──────────────────┬──────────────┘  └────────────────┬──────────────────┘
                   │                                  │
┌──────────────────▼──────────────┐  ┌────────────────▼──────────────────┐
│          Network (iroh)         │  │         crates/crsqlite           │
│  - ALPN: /notare/sync/2         │  │  - Bundled vlcn-io v0.16.3 binary │
│  - QUIC direct / relay (pkarr)  │  │  - crsql_changes virtual table    │
│  - Mutual Ed25519 auth          │  │  - crsql_as_crr / alter windows   │
└─────────────────────────────────┘  └───────────────────────────────────┘
```

The sync stack is composed of the following core modules:

1. **`crates/crsqlite`**: Wraps the upstream cr-sqlite loadable extension (v0.16.3, MIT). Embeds target-specific prebuilt binaries via `include_bytes!` and loads them safely into SQLite connection options. Exposes typed APIs for table conversion (`as_crr`, `is_crr`), alteration windows (`begin_alter`, `commit_alter`), change querying/applying over `crsql_changes`, and statement finalization (`finalize`, `close_pool`).
2. **`crates/db-core`**: Provides `Db::open_crsqlite`, loading cr-sqlite onto pooled SQLite connections, configuring zero-recycle connection pool options, and ensuring proper clean-up in `Db::close`.
3. **`crates/db-app` & `crates/db-migrate`**: Manages the application database schema. Synced tables are governed by `SYNCED_TABLES`, `ELIGIBLE_PENDING_PROOF`, and `NO_GO_TABLES`. DDL operations on synced tables use `MigrationScope::CrrAlter { table_name }` to ensure schema changes run within an alter window on an active connection.
4. **`crates/sync-p2p`**: Implements the transport, protocol framing, device identity, peer allowlisting, and session execution over iroh QUIC bi-streams.
5. **`plugins/db`**: Integrates sync into the desktop application lifecycle. Provides `CrsqliteChangeSource` implementing `sync_p2p::ChangeSource`, manages per-peer progress cursors in `sync_peer_cursors`, and starts `P2pAgent` when sync is enabled by the user.

---

## 2. Engine Loading & Connection Pool Lifecycle

### 2.1 Bundled Extension Packaging (`crates/crsqlite`)

Rather than compiling C/Rust extensions from source during app builds or staging files into `tauri.conf.json` resources (which caused dynamic path resolution issues under issue #154), `crates/crsqlite` packages prebuilt extension binaries directly within the crate:

- Precompiled binaries live at `crates/crsqlite/vendor/<target>/crsqlite.{so,dylib,dll}` across four supported targets: `linux-x86_64`, `darwin-x86_64`, `darwin-aarch64`, and `win-x86_64`.
- Binary integrity is verified at build time by `crates/crsqlite/build.rs` against `vendor/SHA256SUMS`.
- At runtime, `crates/crsqlite::bundled_extension_path()` (`src/bundle.rs`) extracts the embedded bytes into the user's platform cache directory:
  `dirs::cache_dir()/notare/crsqlite/0.16.3/<target>/crsqlite.<ext>`
- Extraction is idempotent: if a file with matching length already exists, extraction is skipped.
- `crates/crsqlite::apply(options)` (`src/lib.rs`) sets up `SqliteConnectOptions` with `.extension(path)`.

### 2.2 Pool Management in `crates/db-core`

When the app opens a database with sync capabilities, it calls `Db::open_crsqlite(options)` (`crates/db-core/src/lib.rs`):

- **No Connection Recycling**: cr-sqlite maintains an internal cache of prepared statements on each SQLite connection handle (`sqlite3*`). Because SQLite refuses to close a connection with active or unfinalized statements (`(code: 5) unable to close due to unfinalized statements`), and sqlx's background worker threads panic on close failure, the connection pool must **never** reap or recycle idle connections in the background. The pool is explicitly configured with:
  ```rust
  pool_options.idle_timeout(None).max_lifetime(None)
  ```
- **Teardown & Statement Finalization**: All connection closes must be coordinated through `Db::close()` (`crates/db-core/src/lib.rs`). If `db.crsqlite_path()` is present, `Db::close()` calls `hypr_crsqlite::close_pool(&self.pool)`, which sweeps every pooled connection, acquires its permit, executes `SELECT crsql_finalize()`, and only then allows SQLite close to proceed.

---

## 3. Synced Tables, CRR Conversion, and Schema Evolution

### 3.1 Synced Table Policy (`crates/db-app`)

Synced-table eligibility is strictly codified in `crates/db-app/src/sync_tables.rs`:

1. **`SYNCED_TABLES`**: The tables actively converted to CRRs:
   - `sessions`, `session_documents`, `transcripts`, `action_items`, `tags`, `session_tags`.
   - Each table has verified convergence proofs demonstrating correct merge behaviour under concurrent edits, updates, and deletes.
2. **`ELIGIBLE_PENDING_PROOF`**:
   - `organizations`, `templates`, `chat_groups`, `chat_messages`.
   - Known to be structurally compatible, but held disabled pending explicit convergence proof verification under cr-sqlite.
3. **`NO_GO_TABLES`**:
   - `humans`, `session_participants`, `calendars`, `events`.
   - **Proven unsafe to sync**: Each of these entities is written behind local `NOT EXISTS (...)` checks using locally minted random UUIDs while the real domain identity lives in non-PK columns (`email`, `(session_id, member)`, `tracking_id_calendar`, `tracking_id_event`). Concurrent creation on multiple offline devices results in permanent duplicate rows that no CRDT merge can reconcile. These tables must not be enabled without deterministic primary key backfills.

### 3.2 On-Demand CRR Conversion (`plugins/db/src/change_source.rs`)

Tables are not converted to CRRs at migration time or app startup. A user who never activates sync remains on standard SQLite tables.
When the user turns sync on, `plugins/db/src/sync.rs` calls `enable_crr_tables(&db)` (`plugins/db/src/change_source.rs`):
- Iterates over `hypr_db_app::SYNCED_TABLES`.
- Executes `hypr_crsqlite::as_crr(pool, table_name)`.
- `crsql_as_crr` is idempotent and creates companion clock tables (`<table>__crsql_clock`). Any pre-existing rows are automatically backfilled into the change tracking stream by cr-sqlite.

### 3.3 Schema Alterations (`crates/db-migrate`)

Modifying the schema of a table that is already a CRR requires wrapping DDL statements in cr-sqlite's alter window.
In `crates/db-migrate`:
- Migrations define `MigrationScope::Plain` or `MigrationScope::CrrAlter { table_name }` (`crates/db-migrate/src/schema.rs`).
- When running a `CrrAlter` step on a CRR, the runner wraps the step on one checked-out connection (`Db::begin_crr_alter_on` / `CrrAlter::commit` in `crates/db-core/src/crr.rs`):
  ```sql
  SELECT crsql_begin_alter('table_name');
  -- execute migration DDL
  SELECT crsql_commit_alter('table_name');
  ```
- If sync is disabled or the table has not yet been converted to a CRR, the step executes as standard DDL without entering the alter window (`Db::begin_crr_alter_on` returns `None`); the window never converts a table.
- **Every window closes connections.** cr-sqlite v0.16.3 caches each connection's view of a CRR's columns and never refreshes it after an alter (probed 2026-11-04): after a committed `ADD COLUMN`, an `UPDATE` fails with `expected 5 values, got 7` on the connection that ran the window and on any connection that wrote to the table before it; only connections opened afterwards see the new schema. So `CrrAlter::commit` finalizes and closes the window's connection and every idle pooled connection, and the runner acquires a fresh one for the next step. A failed DDL or commit closes the window's connection too, which rolls the window (`SAVEPOINT alter_crr`) back — the step stays unapplied and the table stays the same CRR. Pinned in `crates/db-migrate/tests/e2e.rs` (`crsqlite::*`).
- Consequences: alter windows only run while nothing else holds a pooled connection (migrations in `prepare_schema` at open) — `Db::begin_crr_alter_on` refuses to open one while another connection is checked out — and they are refused on in-memory databases, which a fresh connection cannot reach.

---

## 4. Change Source and Wire Caching

### 4.1 The `crsql_changes` Interface (`crates/crsqlite/src/changes.rs`)

The replication stream is produced and consumed via cr-sqlite's virtual table `crsql_changes`:

- **Column Layout (v0.16.3)**:
  `"table", pk, cid, val, col_version, db_version, site_id, cl, seq`
  - `table`: Name of the modified table.
  - `pk`: Packed primary key representation (treated as an opaque byte sequence/BLOB).
  - `cid`: Column name, or `"-1"` indicating a row tombstone (hard DELETE).
  - `val`: Column value.
  - `col_version`: Monotonic version per column.
  - `db_version`: Monotonic transaction counter per database.
  - `site_id`: 16-byte UUID identifying the originating node.
  - `cl`: Causal length (always 1 in v0.16.3).
  - `seq`: Sequence number within the current `db_version` transaction.
- **Value Codec (`SqlValue`)**:
  Values are decoded using `SqliteValueRef::type_info()` to preserve the true SQLite storage class (`Null`, `Integer(i64)`, `Real(f64)`, `Text(String)`, `Blob(Vec<u8>)`). This ensures byte-exact fidelity across nodes without lossy textual conversion.
- **Resumable Tuple Cursor**:
  Changes are ordered by the tuple `(db_version, seq)`. Consecutive pages are fetched with:
  ```sql
  SELECT "table", pk, cid, val, col_version, db_version, site_id, cl, seq
  FROM crsql_changes
  WHERE (db_version, seq) > (?, ?) AND (? IS NULL OR site_id IS NOT ?)
  ORDER BY db_version, seq
  LIMIT ?
  ```
  Excluding the requesting peer's `site_id` prevents echoing changes back to the node that created them.

### 4.2 Application Change Source (`plugins/db/src/change_source.rs`)

`CrsqliteChangeSource` implements `sync_p2p::ChangeSource`:
- **Tracking Peer Cursors**: Tracks each remote peer's synchronization high-water mark in `sync_peer_cursors` (`peer_id TEXT PRIMARY KEY, db_version INTEGER, seq INTEGER, updated_at TEXT`). This is a local SQLite table, never synced.
- **Atomic Batch Apply**: When changes are received, `apply()` converts incoming wire changes into `crsqlite::Change` records and writes them into `crsql_changes` within an explicit SQLite transaction.

---

## 5. Sync Protocol v2 (`crates/sync-p2p/src/protocol.rs`)

Protocol v2 governs communication over bi-directional streams established between devices over iroh.

- **ALPN**: `b"/notare/sync/2"` (`SYNC_ALPN`).
- **Framing**: Every wire message is framed as:
  `[4-byte BE length (max 64 MiB)] [sealed payload]`
- **Sealed Payload**:
  `[24-byte random nonce] [XChaCha20-Poly1305 ciphertext + tag]`
- **Plaintext Format**:
  `[role: u8] [seq: u64 BE] [postcard(Message)]`
  - `role`: `0` for Dialer, `1` for Acceptor.
  - `seq`: Monotonically incrementing per-direction sequence counter starting at `0`.
  - Encoded with `postcard` for compact serialization of binary vectors.
  - Rejects any frame with mismatched role or out-of-order `seq`, preventing replay or reflection attacks.

### 5.1 Protocol Messages

```rust
pub enum Message {
    Hello {
        device_id: String,
        site_id: Vec<u8>,
        proto: u32,
    },
    Pull {
        after: WireCursor,
        limit: u32,
    },
    Changes(WirePage),
    Ack {
        cursor: WireCursor,
    },
    Error {
        code: ErrorCode,
        msg: String,
    },
}
```

---

## 6. Bi-Directional Session Flow (`crates/sync-p2p/src/session.rs`)

Both devices execute synchronization over a single bi-directional stream concurrently:

1. **Handshake**:
   - Both nodes exchange `Message::Hello`.
   - Each side verifies protocol version compatibility (`proto == 2`) and asserts that `device_id` matches the authenticated peer public key from the underlying TLS/iroh connection.
2. **Concurrent Pull & Serve**:
   - **Pull Direction**: The local node reads `source.peer_cursor(peer)`, sends `Pull { after, limit }`, receives `Changes(page)`, applies the changes to SQLite, updates `sync_peer_cursors`, and returns `Ack { cursor }`. The pull loop continues until `page.more == false`.
   - **Serve Direction**: Concurrently, the node answers incoming `Pull` requests from the peer by reading from `source.changes_since(after, exclude_site = peer_site_id, limit)` and transmitting `Changes(page)` until acknowledged.
3. **Graceful Completion**:
   - A direction completes when the final page (`more == false`) is acknowledged via `Ack`. Once both directions have completed, the stream is cleanly closed without dropped frames.

---

## 7. Transport, Identity, and Security

### 7.1 Device Identity (`crates/sync-p2p/src/identity.rs`)

- Each device maintains an Ed25519 keypair persisted at `<data_dir>/notare/sync/device.key` (with strict `0600` permissions on Unix).
- The public key is the device's public identity (represented as a z-base-32 string) and functions directly as the iroh `PublicKey`.

### 7.2 Mutual Allowlist Enforcement (`crates/sync-p2p/src/peers.rs`)

To prevent Unauthorized Dialing and Server-Side Request Forgery (SSRF) risks identified in early audits:
- Allowed peers are stored in `<data_dir>/notare/sync/peers.json`. This file is strictly local and never synchronized.
- Outbound dials only connect to nodes present in `PeerStore`.
- Inbound connections are authenticated against `PeerStore` during the QUIC handshake and re-verified on each bi-stream. Connections from unapproved peers are immediately rejected.

### 7.3 End-to-End Payload Encryption (`crates/sync-p2p/src/crypto.rs`)

All protocol traffic across peers is encrypted end-to-end:
- The device's Ed25519 identity key is converted into an X25519 static Diffie-Hellman key (`ed25519_dalek` Montgomery conversion).
- Diffie-Hellman exchange produces a shared secret with the remote peer's public key.
- A per-peer symmetric key is derived using HKDF-SHA256, binding domain info to both sorted node identities (`pair_domain_info`).
- All frames are authenticated and encrypted using XChaCha20-Poly1305 with random 24-byte nonces. Even if an iroh relay is traversed, relays observe only opaque ciphertext.

### 7.4 Pairing Protocol (`crates/sync-p2p/src/pairing.rs`)

New devices are paired over ALPN `b"/notare/pair/1"` using:
- Short Authentication String (SAS) 6-digit confirmation codes verified mutually by users.
- Mutual exchange of public keys and device metadata, persisted into `PeerStore` on completion.

---

## 8. App Runtime Integration & Platform Gate

### 8.1 Desktop Wiring (`plugins/db/src/sync.rs`)

- Managed by `SyncLifecycle`, held inside `PluginDbRuntime`.
- `SyncLifecycle::start(db)` initializes `CrsqliteChangeSource`, verifies that cr-sqlite is loaded in the database, enables CRR tables, and spawns `P2pAgent`.
- Periodic synchronization runs every 60 seconds across all allowlisted peers.
- Shutdown order in `PluginDbRuntime::shutdown`:
  1. Stop `P2pAgent` (terminating all active network streams).
  2. Stop reactive live-query subscriptions.
  3. Call `Db::close()` (finalizing all cached prepared statements before SQLite closes).

### 8.2 Platform Gate (`build-support/sync_app_gate.rs`)

The P2P sync feature is compiled conditionally using the `sync_platform` cfg:
- **Supported Targets**:
  - `linux` / `x86_64`
  - `macos` / `x86_64`
  - `macos` / `aarch64`
  - `windows` / `x86_64`
- `build-support/sync_app_gate.rs` is included by the `build.rs` scripts in `plugins/db` and `apps/desktop/src-tauri` as the single source of truth.
- `linux/aarch64` is excluded until validated on CI runners.

---

## 30. cr-sqlite Engine Probe Findings and Contract

This section records empirical findings and measurements obtained from testing and probing the vendored **cr-sqlite v0.16.3** binary (`crates/crsqlite/tests/*.rs`). These findings serve as the binding contract for db-core, db-app, and sync-p2p.

### 30.1 Engine Load Probe & Identity (`tests/engine_load.rs`, `src/api.rs`)

1. **No `crsql_version()` Function**:
   - cr-sqlite v0.16.3 does not expose a version function such as `crsql_version()`.
   - **Load Probe**: To verify that the extension is successfully loaded on a connection, `crates/crsqlite::load_probe` executes `SELECT crsql_site_id()`. If the extension is missing, the query errors with `"no such function: crsql_site_id"`.
2. **Site ID & Monotonic Version**:
   - `SELECT crsql_site_id()` returns a 16-byte binary site identifier unique to the database node.
   - `SELECT crsql_db_version()` starts at `0` on a clean database and increments monotonically whenever modifications occur on any CRR-enabled table.

### 30.2 `crsql_changes` Column Layout & Codec (`src/changes.rs`, `tests/engine_changes.rs`)

1. **Column Structure**:
   `crsql_changes` in v0.16.3 exposes exactly 9 columns:
   ```text
   "table", pk, cid, val, col_version, db_version, site_id, cl, seq
   ```
   - `cl` (causal length) is always `1` at this version.
   - `ts` (timestamp) does **not** exist in upstream v0.16.3 (it is a schema modification present only in the Fly.io superfly fork).
2. **Packed Primary Keys**:
   - The `pk` column returns a packed primary key representation stored as a `BLOB`, even for single-column `TEXT PRIMARY KEY` tables. The transport treats this column as an opaque `SqlValue::Blob`.
3. **Hard Deletes**:
   - A hard `DELETE` on a CRR table produces a tombstone row where `cid` is `"-1"`, and `val` is `NULL`.
4. **Storage Class Fidelity (`SqlValue`)**:
   - Decoding values through `SqliteValueRef::type_info()` preserves runtime SQLite storage classes (`INTEGER`, `REAL`, `TEXT`, `BLOB`, `NULL`).
   - Roundtrip tests in `tests/strict_roundtrip.rs` prove byte-exact value and typeof equality for edge values, including `i64::MIN`, `i64::MAX`, `-0.0`, `1e300`, embedded NUL bytes, and 1 MiB binary blobs.

### 30.3 Connection Close, Statement Finalization, and Pool Policy (`tests/close_behaviour.rs`)

1. **sqlx Worker Thread Panic Risk**:
   - sqlx-sqlite executes connection operations on a dedicated OS worker thread.
   - If SQLite fails to close a connection due to active or unfinalized cached statements (`sqlite3_close` returns `SQLITE_BUSY` or `(code: 5) unable to close due to unfinalized statements`), `sqlx`'s `ConnectionHandle::drop` panics.
   - A panic on a sqlx worker thread terminates the process immediately.
2. **Statement Caching**:
   - cr-sqlite caches internal prepared statements per connection handle and provides `SELECT crsql_finalize()` to clear this cache.
3. **Close Probe Scenarios**:
   - *Scenario 1*: A pool with `idle_timeout(100ms)` and no finalization was tested. Connections reaped by the pool panic the worker thread on close.
   - *Scenario 2*: A pool with `idle_timeout(100ms)` executing `crsql_finalize()` on each connection before drop.
   - *Scenario 3 (Adopted Policy)*: `idle_timeout(None)` and `max_lifetime(None)` (disabling background connection reaping), combined with a dedicated `finalize_all` sweep before pool closure. Scenario 3 successfully reached `SURVIVED`.
4. **Enforced App Pool Policy**:
   - `Db::open_crsqlite` configures `idle_timeout(None)` and `max_lifetime(None)`.
   - `hypr_crsqlite::close_pool` and `Db::close` acquire every pooled connection and run `SELECT crsql_finalize()` before allowing the connections to close.

### 30.4 Schema Compatibility and STRICT Table Rules (`tests/strict_roundtrip.rs`)

1. **TEXT Primary Keys Require `NOT NULL`**:
   - In SQLite `STRICT` mode, cr-sqlite rejects tables defined with `INTEGER PRIMARY KEY` unless explicitly declared `NOT NULL`.
   - All Notare synced tables standardize on `id TEXT PRIMARY KEY NOT NULL`.
2. **`crsql_as_crr` Idempotency**:
   - Invoking `SELECT crsql_as_crr('table_name')` multiple times on the same table is safe and idempotent.
3. **Engine Rejection Exceptions**:
   - `voice_profiles`: Rejected by `crsql_as_crr` with `"NOT NULL column without a DEFAULT"` because it contains required columns lacking default values.
   - `embedding_vector_map`: Rejected by `crsql_as_crr` with `"unique indices"` because cr-sqlite forbids secondary unique indices on synced tables.

### 30.5 Remote Apply Update Hook Probe (`tests/strict_roundtrip.rs`)

1. **`sqlite3_update_hook` Does Not Fire on Remote Apply**:
   - `tests/strict_roundtrip.rs::remote_apply_update_hook_probe` evaluated whether writing incoming peer changes into the `crsql_changes` virtual table triggers SQLite's `sqlite3_update_hook`.
   - **Finding**: Virtual table inserts into `crsql_changes` do **not** trigger `sqlite3_update_hook` callbacks.
2. **Reactive Query Implications**:
   - Notare's live query mechanism (`db-reactive` / `ChangeNotifier`) does not automatically detect remote synchronization updates for free.
   - The sync engine must explicitly trigger query invalidation / refresh notifications when a sync batch completes.

---

## Superseded Historical Record (§1–§23)

> **HISTORICAL NOTICE**: Sections 1 through 23 of the original sync specification detailed the CloudSync / sqlite-sync (Elastic License 2.0) C network-layer contract, the S1 transport spike, and development increments SYNC-3 through SYNC-9. Following SQLite Cloud's written denial of our custom network layer licence on 2026-09-02, the entire CloudSync C architecture was superseded and completely excised in release 0.6.7.
>
> The complete historical record has been preserved in `docs/internal/archive/sync-p2p-cloudsync-era.md`.
>
> **Do not attempt to execute or implement the build recipes, verification commands, or contracts from the archived sections.**

For convenience, the topics covered in the archived record and their modern equivalents are indexed below:

| Historical Section | Original Topic (CloudSync Era) | Current State in 0.6.7+ / 0.7 |
|---|---|---|
| §1–§6 | `network_p2p.c` C functions (`network_send_buffer`, `network_receive_buffer`), `NETWORK_RESULT`, memory allocation rules | **Removed.** Replaced by pure Rust `crates/sync-p2p`. |
| §7–§10 | C build recipe (`CLOUDSYNC_OMIT_CURL`), feature flags, verification commands (`cargo check -p cloudsync`) | **Removed.** C compilation flags and `from-source` features deleted. |
| §11–§12 | S1 transport spike, localhost-TCP broker, C network audit | **Removed.** Replaced by iroh QUIC transport. |
| §13–§14 | SYNC-3: iroh P2P transport, device identity, peer allowlist | Architecture carried forward to `crates/sync-p2p` (see §7 above). |
| §15 | SYNC-4: N-way convergence and elected hub | Replaced by direct P2P mesh sessions over Protocol v2. |
| §16 | SYNC-5: App wiring, bearer token localhost socket | **Removed.** Replaced by in-process `SyncLifecycle` in `plugins/db`. |
| §17 | SYNC-6 part A: Real-schema convergence proofs for `sessions` | Convergence principles retained; see §30 for cr-sqlite verification. |
| §18 | SYNC-7: E2E payload encryption (X25519 + XChaCha20-Poly1305) | Carried forward into Protocol v2 framing (`crates/sync-p2p/src/crypto.rs`). |
| §19 | SYNC-6 part B: Table enable-sets and SAS device pairing | Carried forward into `crates/sync-p2p/src/pairing.rs`. |
| §20 | SYNC-8: Real discovery via iroh relay and DNS/pkarr | Carried forward in `crates/sync-p2p/src/agent.rs`. |
| §21 | SYNC-9: Cross-platform `from-source` C builds (Winsock2, macOS) | **Removed.** No C sources remain; `crates/crsqlite` bundles prebuilt binaries. |
| §22 | App-gate widening across desktop platforms | Maintained in `build-support/sync_app_gate.rs` (see §8.2 above). |
| §23 | Table proofs lane (`transcripts`, `action_items`, `tags`, `session_tags`) | Carried forward into `crates/db-app/src/sync_tables.rs`. |
