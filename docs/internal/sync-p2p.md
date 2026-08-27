# CloudSync P2P sync — C network-layer contract

Spike **S0b** deliverable. This documents the verbatim C contract between the
vendored sqlite-sync (CloudSync) extension v1.0.12 and the custom network
layer that the follow-on transport spike (**S1**) must implement.

It is the source of truth for the signatures — **the brief's signatures were
stale** (they described an older/newer shape with `extra_headers`/`nextra_headers`
and an enum-typed `NETWORK_RESULT`). The real v1.0.12 source differs. Capture
here is verbatim from the vendored source.

## Provenance

| Field | Value |
|---|---|
| Upstream repo | https://github.com/sqliteai/sqlite-sync |
| Tag | `1.0.12` |
| Commit SHA | `6694c2e8b084d6f33d8bf86742ac1f2b8243bd6e` |
| Vendored at | `crates/cloudsync/vendor/src/` (see `UPSTREAM.md` there) |
| License | Elastic License 2.0 (modified for open-source use) + MIT submodule |

---

## 1. The two functions S1 must implement

When the extension is built with `-DCLOUDSYNC_OMIT_CURL`, the default
libcurl implementation of these two functions is excluded from compilation
(see `src/network/network.c`, `#ifndef CLOUDSYNC_OMIT_CURL` … `#endif`), but
the CloudSync core still *calls* them. A shared object with undefined symbols
fails to `dlopen` (SQLite loads extensions with `RTLD_NOW`), so any from-source
build must provide at least these two symbols. They are the **sole** replace
point for S1's P2P transport — swap `crates/cloudsync/build/network_stub.c`
for the real implementation, nothing else in the core changes.

Verbatim from `src/network/network_private.h`:

```c
bool network_send_buffer(network_data *data, const char *endpoint, const char *authentication, const void *blob, int blob_size);

NETWORK_RESULT network_receive_buffer (network_data *data, const char *endpoint, const char *authentication, bool zero_terminated, bool is_post_request, char *json_payload, const char *custom_header);
```

**Calling convention:** plain C (default cdecl on linux/x86_64). No `extern
"C"`, no `__attribute__`, no `JNIEXPORT`, no `SQLITE_API`. They are plain
file-scope functions compiled as C; the core declares them in
`network_private.h` and links them at shared-object load time.

> ⚠️ The brief listed the second function with a trailing
> `const char **extra_headers, int nextra_headers`. That does **not** match
> v1.0.12. The real last parameter is a single `const char *custom_header`
> (one extra HTTP header string, or NULL). S1 must implement the verbatim
> signature above.

## 2. `NETWORK_RESULT` (it is a STRUCT, not an enum)

Verbatim from `src/network/network_private.h`:

```c
#define CLOUDSYNC_NETWORK_OK                1
#define CLOUDSYNC_NETWORK_ERROR             2
#define CLOUDSYNC_NETWORK_BUFFER            3

typedef struct network_data network_data;

typedef struct {
    int     code;                   // network code: OK, ERROR, BUFFER
    char    *buffer;                // network buffer
    size_t  blen;                   // blen if code is SQLITE_OK, rc in case of error
    void    *xdata;                 // optional custom external data
    void    (*xfree) (void *);      // optional custom free callback
} NETWORK_RESULT;
```

The `code` field takes one of the three `CLOUDSYNC_NETWORK_*` integer macros
above. Despite the comment on `blen`, the default impl sets:
- `OK` (1): `buffer`/`blen` unused — means "request succeeded, no body".
- `BUFFER` (3): `buffer` points at the response body, `blen` is its byte length.
- `ERROR` (2): `buffer` may hold a diagnostic string; `blen` is set to the
  curl/libc error code (`rc`).

> ⚠️ The brief called `NETWORK_RESULT` an enum. It is a **struct returned by
> value**. S1 returns `(NETWORK_RESULT){...}` compound literals.

## 3. `network_data` (opaque in the header, concrete in network.c)

The public header only exposes the opaque typedef:
`typedef struct network_data network_data;`. The concrete definition lives
in `src/network/network.c` (compiled into the same shared object, so the
core and the custom network layer share it):

```c
struct network_data {
    char        site_id[UUID_STR_MAXLEN];   // UUID_STR_MAXLEN == 37
    char        *authentication;            // apikey or token
    char        *org_id;                    // organization ID for X-CloudSync-Org header
    char        *check_endpoint;
    char        *upload_endpoint;
    char        *apply_endpoint;
    char        *status_endpoint;
};
```

Accessors provided by the core (declared in `network_private.h`, implemented
in `network.c`, available to the custom layer):

```c
char *network_data_get_siteid (network_data *data);
char *network_data_get_orgid  (network_data *data);
bool  network_data_set_endpoints (network_data *data, char *auth, char *check, char *upload, char *apply, char *status);
```

S1 may read `data->site_id` / `data->org_id` via the accessors to identify the
local site, and read the `*_endpoint` fields directly (they are set by the
core's `network_compute_endpoints_with_address`, see §5).

## 4. Memory ownership — the one rule S1 must not get wrong

- **Returned buffers must be allocated with `cloudsync_memory_zeroalloc`**
  (or `cloudsync_string_dup`), which resolves to the SQLite allocator
  (`dbmem_*` → `sqlite3_malloc`/`sqlite3_free`) for the SQLite build. The
  caller frees them.

- The canonical free path is the core's `network_result_cleanup()` (in
  `network.c`):
  ```c
  void network_result_cleanup (NETWORK_RESULT *res) {
      if (res->xfree) {
          res->xfree(res->xdata);
      } else if (res->buffer) {
          cloudsync_memory_free(res->buffer);
      }
  }
  ```
  So: if S1 allocates `buffer` with `cloudsync_memory_*`, leave `xfree`/`xdata`
  NULL and the caller frees via `cloudsync_memory_free` (= `sqlite3_free`).
  If S1 uses a *different* allocator for the buffer (e.g. a platform-native
  one), it must set `xfree` to the matching free callback and `xdata` to the
  allocation, so `network_result_cleanup` calls the right free. **Never**
  return a buffer allocated with plain `malloc` while leaving `xfree` NULL —
  the caller would `sqlite3_free` it and corrupt the heap.

- `network_send_buffer` returns `bool` only — it owns and frees its own
  scratch (the default impl frees the curl handle + slist internally). The
  `blob`/`blob_size` are borrowed from the caller for the duration of the
  call and are *not* freed by the send function.

## 5. What `endpoint` and `authentication` mean (free-form C strings)

Both are **free-form, NUL-terminated C strings** owned by the caller (the
`network_data` struct). S1 receives them as `const char *` and must not free
them.

- **`endpoint`** — an absolute URL. The core builds it in
  `network_compute_endpoints_with_address` as
  `{address}/v2/cloudsync/databases/{managedDatabaseId}/{siteId}/{action}`
  where `action` ∈ `check` / `upload` / `apply` / `status`
  (`CLOUDSYNC_ENDPOINT_*` in `network_private.h`). For P2P, `address` is set
  via `cloudsync_network_init_custom(address, managedDatabaseId)` — S1 can
  **map a peer address onto `endpoint`** freely; the core treats it as an
  opaque URL it hands to the network layer. A non-HTTP scheme (e.g.
  `p2p://…`, `ws://…`) is acceptable as long as S1's transport parses it.
  `address`/`managedDatabaseId` are user-supplied via SQL, so they are
  attacker-influenced input — S1 must parse defensively.

- **`authentication`** — the apikey or bearer token, set via
  `cloudsync_network_set_token` / `cloudsync_network_set_apikey` (stored in
  `data->authentication`). May be NULL (e.g. the upload step passes NULL auth
  because it targets a pre-signed S3 URL). Free-form string; S1 can encode a
  peer credential / shared secret in it.

- **`custom_header`** — one extra header string (format `"Name: value"`) or
  NULL. The core passes `CLOUDSYNC_HEADER_SQLITECLOUD` (`"Accept: sqlc/plain"`)
  for most calls. S1 may ignore it for a non-HTTP transport.

- **`json_payload`** — POST body string (only when `is_post_request == true`),
  or NULL. For `is_post_request == true` with NULL payload, the default impl
  sends an empty POST.

- **`zero_terminated`** — if true, the returned `buffer` must be
  NUL-terminated (the core sets this true for every real call site). S1
  should always NUL-terminate to be safe.

- **`is_post_request`** — true ⇒ POST, false ⇒ GET.

## 6. Blocking vs non-blocking

The core calls these functions **synchronously** from SQLite SQL-function
context (e.g. `SELECT cloudsync_network_check_changes()`). They block the
calling thread until the request completes. The default libcurl impl is fully
blocking (`curl_easy_perform`). **S1's transport must also block** — there is
no async/callback contract. `network_receive_buffer` returns the complete
`NETWORK_RESULT` by value; `network_send_buffer` returns `bool`. S1 must not
defer completion to a later event-loop tick.

The core's `cloudsync_network_sync` wraps calls with `sqlite3_sleep(wait_ms)`
retries, but that is outside the network layer — S1 just needs to block per
call.

## 7. Build recipe: `CLOUDSYNC_OMIT_CURL`

- Define the preprocessor macro `CLOUDSYNC_OMIT_CURL` for **every** C
  translation unit of the extension. In `crates/cloudsync/build.rs` this is
  done via `cc::Build::define("CLOUDSYNC_OMIT_CURL", None)` on each unit.
- This excludes: the two function bodies, `curl_global_init/cleanup`, and the
  `#include "curl/curl.h"` (all inside `#ifndef CLOUDSYNC_OMIT_CURL`).
- It does **not** exclude the rest of `network.c` — the JSON helpers, the
  `network_data` struct + accessors, `network_result_cleanup`,
  `network_set_sqlite_result`, the sync orchestration
  (`cloudsync_network_send_changes_internal`, `cloudsync_network_check_internal`,
  `cloudsync_network_sync`, etc.), and `cloudsync_network_register` all
  compile normally and call *your* `network_send_buffer`/`network_receive_buffer`.

- A separate macro `CLOUDSYNC_OMIT_NETWORK` exists but is **never** set here —
  it would drop the entire network module including registration. Do not set
  it. (`network.c` and `cloudsync_sqlite.c` guard on it.)

- cURL references outside `network.c`: only `src/network/cacert.h` (an
  Android-only PEM bundle, pulled in only under `__ANDROID__`, never on
  linux). No other source unit references curl symbols, so `CLOUDSYNC_OMIT_CURL`
  cleanly drops libcurl with no dangling references on linux/x86_64.

### Build system facts

- Upstream uses a plain `Makefile` (no CMake). C standard: C11 (the code uses
  compound literals, `stdbool.h`, designated initializers). `cc` defaults to
  a compatible standard.
- Include paths needed: the vendor root, `network/`, `sqlite/`,
  `modules/fractional-indexing/` (the build.rs sets all four).
- Link: `-shared -o cloudsync.so <objects> -lm`. No libcurl, no libcrypto —
  the extension vendors its own LZ4, jsmn, khash, and uses SQLite's allocator.
- Compile flags: `-fPIC -O2 -DCLOUDSYNC_OMIT_CURL`. Warnings are off in
  build.rs (upstream code is not warning-clean under strict flags).
- The `.so` is **loadable**, not linked into the Rust binary — build.rs
  invokes the linker manually (not `cc::Build::compile`) to avoid emitting
  `cargo:rustc-link-lib` directives that would wrongly link the extension
  into the host. The path is passed to the crate via
  `cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO=<OUT_DIR>/cloudsync.so`, and
  `bundle.rs` reads it at runtime under `#[cfg(feature = "from-source")]`.

### Per-platform build outputs (only linux/x86_64 proven in S0b)

| Target | Output | Status |
|---|---|---|
| linux/gnu/x86_64 | `cloudsync.so` | ✅ Proven (S0b): builds, loads, `cloudsync_version() == "1.0.12"` |
| linux/musl/x86_64 | `cloudsync.so` | Not built (build.rs panics under `from-source`; falls through to prebuilt) |
| linux/aarch64 | `cloudsync.so` | Not built — SYNC-9 |
| macos (aarch64/x86_64) | `cloudsync.dylib` | Not built — uses upstream `network.m` (NSURLSession), SYNC-9 |
| android (arm*/x86_64) | `cloudsync.so` | Not built — needs `cacert.h`, SYNC-9 |
| windows/x86_64 | `cloudsync.dll` | Not built — SYNC-9 |

## 8. Feature gating

`crates/cloudsync/Cargo.toml` declares a default-OFF feature:

```toml
[features]
from-source = []

[build-dependencies]
cc = "1.2"
```

- **Feature OFF (default):** `build.rs` is a no-op; `bundle.rs` uses the
  existing `include_bytes!` prebuilt-artifact path byte-identical to `main`.
  Verified: `cargo check -p cloudsync` clean.
- **Feature ON (`--features from-source`):** `build.rs` compiles the vendored
  source + `build/network_stub.c` into `OUT_DIR/cloudsync.so` and exports
  `CLOUDSYNC_FROM_SOURCE_SO`; `bundle.rs` returns that path instead of the
  cache-dir prebuilt path. Verified: `cargo test -p cloudsync --features
  from-source` passes (`loads_bundled_cloudsync`, asserts version ==
  `CLOUDSYNC_VERSION`).

## 9. For S1 — the transport spike

**Implement exactly these two functions** in a new file replacing
`crates/cloudsync/build/network_stub.c` (keep the file name or add a new one
to `build.rs`'s `sources`/stub list):

```c
#include "network_private.h"

bool network_send_buffer(network_data *data, const char *endpoint,
                          const char *authentication, const void *blob, int blob_size);

NETWORK_RESULT network_receive_buffer(network_data *data, const char *endpoint,
                          const char *authentication, bool zero_terminated,
                          bool is_post_request, char *json_payload, const char *custom_header);
```

### Sharp edges for S1

1. **`NETWORK_RESULT` is a struct returned by value.** Return it via compound
   literal `(NETWORK_RESULT){code, buffer, blen, xdata, xfree}` (field order
   matters: `code`, `buffer`, `blen`, `xdata`, `xfree`). The brief's "enum"
   description was wrong.

2. **The second function's last param is `const char *custom_header`** (one
   header), **not** `extra_headers`/`nextra_headers`. The brief was wrong here
   too. Match the verbatim signature.

3. **Buffer allocator must match the free path.** Default: allocate with
   `cloudsync_memory_zeroalloc` and leave `xfree`/`xdata` NULL → caller frees
   with `cloudsync_memory_free` (= `sqlite3_free`). If you use a different
   allocator, set `xfree` to its free fn and `xdata` to the allocation. Never
   mix `malloc`-allocated buffer with a NULL `xfree`.

4. **Always NUL-terminate** `buffer` when `zero_terminated == true` (every real
   call site sets it true). Allocate `blen + 1` and write `0` at `blen`.

5. **Blocking only.** No callbacks, no async. Block until the transport
   completes; return the full result. The core calls you from a SQLite
   function context on the DB thread.

6. **`endpoint` is free-form.** S1 can map a P2P peer address onto it. But
   `address`/`managedDatabaseId` (and thus `endpoint`) are user-supplied via
   SQL — **untrusted input**. Parse defensively; reject/`ERROR` on bad
   schemes rather than crashing.

7. **`authentication` may be NULL** (the upload step passes NULL auth to a
   pre-signed URL). Handle NULL gracefully in both functions.

8. **`network_send_buffer` semantics:** the default impl does an HTTP **PUT**
   (`CURLOPT_UPLOAD`) of the raw `blob` (`application/octet-stream`) to the
   `endpoint` URL (an S3 pre-signed URL returned by the prior `receive` call
   to the upload endpoint). It returns `true` on success. S1's P2P transport
   must produce the same effect — deliver the `blob` bytes to wherever the
   `endpoint`'s URL instructs — or redefine the endpoint contract to mean a
   direct peer PUT. The core's flow is: `receive(upload_endpoint)` → parse
   JSON `{"url":...}` → `send_buffer(that_url, NULL, blob)` →
   `receive(apply_endpoint, POST json)`.

9. **The two-step upload flow is HTTP-S3-shaped.** The default protocol uses a
   pre-signed-URL indirection: check/upload return a JSON `{"url": ...}`
   pointing at object storage, then `network_send_buffer` PUTs the blob there,
   then an `apply` POST notifies the server. For true P2P, S1 will likely
   **collapse this** — implement `network_receive_buffer` against the
   `upload`/`check`/`apply`/`status` endpoints to talk directly to a peer
   serving the CloudSync protocol, and make `network_send_buffer` push the
   blob straight to the peer. The endpoint URL format is S1's to define as
   long as the core's `network_compute_endpoints_with_address` template still
   produces strings S1 can route on. Consider whether S1 needs a custom
   `cloudsync_network_init_custom` address scheme.

10. **Rebuild is automatic.** `build.rs` emits `cargo:rerun-if-changed` for
    the vendor dir and the stub, so editing the stub triggers a rebuild. S1
    iterates by editing the stub file and re-running
    `cargo test -p cloudsync --features from-source`.

11. **Don't set `CLOUDSYNC_OMIT_NETWORK`.** It would drop registration and
    break `cloudsync_network_*` SQL functions. Only `CLOUDSYNC_OMIT_CURL` is
    set.

12. **`cloudsync_version()` comes from the core**, not the network layer.
    `CLOUDSYNC_VERSION` ("1.0.12") is a macro in `cloudsync.h`, surfaced via
    the `cloudsync_version()` SQL function. S1 does not touch it; it is the
    load-success smoke check.

## 10. Verification (S0b)

```
$ cargo check -p cloudsync                       # feature OFF → clean
   Finished `dev` profile in 1.65s

$ cargo test -p cloudsync --features from-source # feature ON
   Compiling cloudsync v0.1.0 ...
   Finished `test` profile in 4.75s
test tests::loads_bundled_cloudsync ... ok
test result: ok. 1 passed; 0 failed

$ nm -D .../cloudsync.so | grep network_send_buffer
00000000000321d0 T network_send_buffer
$ nm -D .../cloudsync.so | grep network_receive_buffer
00000000000321e0 T network_receive_buffer
$ nm -D .../cloudsync.so | grep -ic curl
0
```

---

# 11. S1 transport spike — call graph + endpoint scheme + GO proof

Appended by S1 (transport-spike engineer). Source-verified against the
vendored v1.0.12 source (`src/network/network.c`, `src/cloudsync.c`,
`src/sqlite/cloudsync_sqlite.c`). This section is the authoritative
description of the network call sequence the custom transport must satisfy,
the endpoint scheme S1 defined, and the convergence proof.

## 11.1 The core's exact network call sequence

`cloudsync_network_sync()` (`network.c:1027`) is the top-level sync entry
(registered as the `cloudsync_network_sync` SQL function). It runs two phases:

**Phase A — send (`cloudsync_network_send_changes_internal`, `network.c:856`):**

1. `cloudsync_payload_get` (`cloudsync.c:3239`) builds a blob of all rows with
   `db_version > settings.send_dbversion` (per-site-local watermark). Returns
   `blob`, `blob_size`, `db_version` (old watermark), `new_db_version` (max).
2. If the blob is empty **and** `db_version == 0` → skip network entirely
   (Case 1, `network.c:877`).
3. If there IS a blob:
   - **`receive(upload_endpoint, GET, auth, zero_term=true, is_post=false, body=NULL)`** (`network.c:889`)
     - Expects `CLOUDSYNC_NETWORK_BUFFER` with JSON `{"url": "..."}`.
     - `json_extract_string(buf, "url")` → `s3_url` (NULL → error "missing 'url'").
   - **`send_buffer(s3_url, auth=NULL, blob, blob_size)`** (`network.c:904`)
     - The default impl HTTP-PUTs the blob to `s3_url`. Returns `bool`.
   - Build `json_payload = {"url":"<s3_url>", "dbVersionMin":<db_version+1>, "dbVersionMax":<new_db_version>}` (`network.c:917`).
   - **`receive(apply_endpoint, POST, auth, zero_term=true, is_post=true, body=json_payload)`** (`network.c:924`)
     - Expects `BUFFER` with `{"lastOptimisticVersion":N, "lastConfirmedVersion":N, "gaps":[...]}`.
4. If there is NO blob (nothing to send):
   - **`receive(status_endpoint, GET, auth, zero_term=true, is_post=false, body=NULL)`** (`network.c:928`)
5. Parse the apply/status response: `lastOptimisticVersion`, `lastConfirmedVersion`, `gaps` size. Update `settings.send_dbversion` from `lastOptimisticVersion` (or `new_db_version` if absent) (`network.c:946-956`).

**Phase B — check/receive (`cloudsync_network_check_internal`, `network.c:983`):**

1. Read `settings.check_dbversion` and `settings.check_seq`.
2. Build `json_payload = {"dbVersion":<check_dbversion>, "seq":<check_seq>}` (`network.c:998`).
3. **`receive(check_endpoint, POST, auth, zero_term=true, is_post=true, body=json_payload)`** (`network.c:1000`)
   - If `code == CLOUDSYNC_NETWORK_BUFFER`:
     - `json_extract_string(buf, "url")` → `download_url` (NULL → **error "missing 'url' in check response"**, `network.c:1005`).
     - `network_download_changes(context, download_url, &nrows)` (`network.c:1009`):
       - **`receive(download_url, GET, auth=NULL, zero_term=false, is_post=false, body=NULL)`** (`network.c:428`)
       - Expects `BUFFER` with the raw changes blob bytes.
       - `cloudsync_payload_apply` (`cloudsync.c:3021`) decodes + CRDT-merges each row, advancing `check_dbversion`/`check_seq` from the last decoded row (`cloudsync.c:3204-3214`).
   - Else (`code != BUFFER`, incl. `CLOUDSYNC_NETWORK_OK`): `network_set_sqlite_result` → success, no download (`network.c:1011`). **This is the "no changes" path** — the server signals "nothing new" by returning OK with no body, NOT by returning a null url.
4. `cloudsync_network_sync` loops phase B up to `max_retries` (default 1), breaking when `nrows > 0` (`network.c:1034-1040`).

**Status-only calls** (not part of sync): `cloudsync_network_status` (`network.c:1180`) and `cloudsync_network_has_unsent_changes` (`network.c:840`) both call `receive(status_endpoint, GET, auth, ...)` and parse `lastOptimisticVersion`.

### Key shapes the transport MUST produce

| call | response the core expects |
|---|---|
| `receive(upload)` | `BUFFER` `{"url":"<string>"}` (must be a JSON string) |
| `send_buffer(url)` | `bool` true |
| `receive(apply)` | `BUFFER` `{"lastOptimisticVersion":N,"lastConfirmedVersion":N,"gaps":[]}` |
| `receive(check)` (changes available) | `BUFFER` `{"url":"<string>"}` |
| `receive(check)` (nothing new) | `OK` (code 1, **no body**) — NOT `{"url":null}` |
| `receive(download_url)` | `BUFFER` raw blob bytes |
| `receive(status)` | `BUFFER` `{"lastOptimisticVersion":N,"lastConfirmedVersion":N,"gaps":[]}` |

> Sharp edge discovered by S1: the core's `check_internal` does NOT gracefully
> handle a `null` url — `json_extract_string` returns NULL for a `JSMN_PRIMITIVE`
> `null`, hitting the "missing 'url'" error (`network.c:1005`). The correct
> "nothing new" signal is `CLOUDSYNC_NETWORK_OK` (no buffer at all), which the
> core routes to `network_set_sqlite_result` → success.

## 11.2 The endpoint scheme S1 defined

`cloudsync_network_init_custom(address, managedDatabaseId)` builds endpoints as:
```
{address}/v2/cloudsync/databases/{managedDatabaseId}/{siteId}/{action}
```
S1 sets `address = "p2p://127.0.0.1:<broker_port>"` (a non-HTTP scheme — the
core treats `address`/`endpoint` as opaque, per §5). The four actions
(`check`/`upload`/`apply`/`status`) map onto the broker directly. The S3
pre-signed-URL indirection is collapsed: the broker serves `{"url":"mem://..."}`
itself and holds the blob in an in-memory object store keyed by `mem://` URLs.

**`mem://` URLs carry the broker address:** `mem://127.0.0.1:<port>/<id>`. This
is necessary because `network_send_buffer` is called with the `mem://` URL
*and no other context* — the C layer must recover host:port from the URL alone
(the `network_data` struct's endpoint fields are not exposed to the custom
layer; only `network_data_get_siteid`/`get_orgid`/`set_endpoints` are). So the
broker embeds its own `host:port` in every minted `mem://` URL.

## 11.3 How the S3 3-step flow maps to direct peer serving

The S3 flow (`receive(upload)` → `send_buffer(s3_url)` → `receive(apply)`) is
**cleanly collapsible** — confirmed GO. The broker IS the S3 server + the
CloudSync control plane in one process:

1. `receive(upload)` → broker mints a `mem://` URL, returns `{"url":"mem://..."}`.
2. `send_buffer(mem://...)` → C layer base64-PUTs the blob to the broker, broker stores it under that URL.
3. `receive(apply)` → broker moves the blob into the per-database change log (keyed by a broker-assigned sequence), bumps `lastOptimisticVersion`, returns the status JSON.
4. `receive(check)` → broker serves the next un-pulled blob from the log (per-site high-water mark), returns `{"url":"mem://..."}` pointing at a fresh copy.
5. `receive(download_url)` → broker returns the raw blob bytes; the core's `cloudsync_payload_apply` CRDT-merges them.

The blob is opaque to the broker — it is the encoded CRDT changeset. The
CRDT merge happens entirely inside the sqlite-sync core on each peer.

> NO-GO was not hit. The 3-step collapse is clean: the only transport-visible
> contract is `{"url":"..."}` JSON + raw blob bytes, both of which a peer can
> serve directly with no S3.

## 11.4 The convergence proof (GO)

`cargo run -p sync-p2p --example sync_two_nodes` — two independent file-backed
SQLite DBs, two independent cloudsync site IDs, `cls` (CausalLengthSet) CRDT on
a `notes(id INTEGER PRIMARY KEY, body TEXT)` table, syncing through one broker:

```
[broker] listening at p2p://127.0.0.1:<port>
[nodes] A and B initialized; cloudsync enabled on 'notes'
[A] wrote 2 rows
[A] sync -> broker: {"send":{"status":"synced","localVersion":2,"serverVersion":2},"receive":{"rows":2,"tables":["notes"]}}
[B] check <- broker: {"receive":{"rows":2,"tables":["notes"]}}
[conv] A -> B OK
[B] wrote 1 row
[conv] B -> A OK
[both] updated row 1 concurrently
[conv] concurrent update converged (row 1 = "B wins" on both)

=== S1 GO: two-node convergence over custom P2P transport ===
```

- **A → B**: 2 rows written on A, `cloudsync_network_sync()`, then
  `cloudsync_network_check_changes()` on B → B has both rows, bodies match.
- **B → A**: 1 row written on B, sync → A has the third row, body matches.
- **Concurrent update**: both update row 1 to different values, 3 sync rounds
  → both converge to the same value (CRDT last-write-wins by causal-length /
  site-id tiebreak). **Conflict-free.**

## 11.5 Memory / threading sharp edges S1 hit

1. **`NETWORK_RESULT` buffers must use `cloudsync_memory_zeroalloc`** + leave
   `xfree`/`xdata` NULL (the core frees via `cloudsync_memory_free` =
   `sqlite3_free`). S1's C layer allocates response bodies with
   `cloudsync_memory_zeroalloc` and NUL-terminates (alloc `len+1`, write 0 at
   `len`). Verified: no heap corruption across the full two-node run.
2. **`cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO=...` from cloudsync's build.rs
   does NOT propagate to `std::env::var` in a *different* crate's binary.**
   cloudsync's own unit test works (the var is baked into its crate), but the
   `sync_two_nodes` example (a separate binary) cannot read it at runtime. S1
   worked around this by giving `crates/sync-p2p` its own `build.rs` that
   rebuilds the `.so` and emits the env var for the example. (Production fix:
   `bundle.rs` should use `env!` not `std::env::var`, or the path should be
   resolved via a `DEP_CLOUDSYNC_*` metadata link.)
3. **CloudSync context is per-connection.** `cloudsync_init`/`cloudsync_enable`
   register the table in a context attached to the SQLite connection. A sqlx
   pool with `max_connections > 1` routes the `INSERT` (which fires the
   cloudsync trigger) to a different connection that has no cloudsync context →
   "Unable to retrieve table name". Fix: `max_connections(1)`.
4. **The broker must retain ALL uploaded blobs, not just the latest.** The
   core's `db_version` is **per-site-local** — two peers can both be at
   db_version 3 for different changes, so `db_version_max <= db_version`
   cannot order cross-site changes. The real server uses a global
   server-assigned db_version; the spike approximates it with a per-database
   upload log + per-site delivery high-water marks (broker-assigned sequence).
   A "latest blob only" broker loses intermediate peer changes and fails
   concurrent-update convergence.
5. **"No changes" = `OK` (no body), not `{"url":null}`.** See §11.1 sharp edge.
6. **Synchronous blocking only.** The C functions block on a fresh TCP
   connection per call (connect → write frame → read frame → close). No async
   runtime lives in the C layer; the broker's tokio runtime is a separate
   process concern. The core calls these from a SQLite function context on the
   DB thread; blocking is correct.

## 11.6 What production hardening (v0.6) needs to know

- **iroh/QUIC replaces TCP.** The framed protocol (`crates/sync-p2p/src/protocol.rs`)
  is transport-agnostic — swap the `TcpStream` for an iroh node. The broker's
  endpoint scheme (`p2p://`) already anticipates a non-HTTP, peer-addressable
  scheme. NAT traversal / relay is iroh's job; the spike deliberately uses
  localhost to prove convergence, not NAT punching.
- **Encryption + auth.** The spike is plaintext + no auth (broker trusts
  localhost). `authentication` (the apikey/token string) is passed through
  untouched and can carry a peer credential / shared secret; the broker should
  verify it. `endpoint` is attacker-influenced — the C layer parses defensively
  and rejects bad schemes.
- **The broker is a relay, not a merge authority.** It only shuttles opaque
  blobs; CRDT merge is per-peer in the sqlite-sync core. Production with
  >2 peers needs the broker's per-database log + per-site delivery to scale
  (or a real global db_version from a coordination service). The spike's
  per-site high-water mark is O(peers × blobs) and unbounded — production
  needs GC of delivered blobs.
- **Relay vs direct peer.** The spike uses a star topology (both peers → one
  broker). True P2P (peer serves peer directly) means each device runs the
  broker logic; the `mem://` object store + control endpoints are the same.
  iroh's direct-connect / relay-fallback maps onto this naturally.
- **Pairing / discovery.** Out of scope; `cloudsync_network_init_custom` takes
  the address as a SQL arg, so discovery just needs to produce that string.
- **`bundle.rs` env-var fix** (§11.5 #2) before the from-source build is used
  by any non-cloudsync binary in the real app.

---

## 12. Audit outcome (2026-08-27) — `network_p2p.c`

Adversarial panel via the `auditor` skill, 4 seats over two runs (coder =
glm-5.2, so the glm family was excluded from every panel). Seats:
`mistral-large-3`, `nemotron-3-ultra`, `gpt-oss:120b`, `kimi-k2.7-code`.
`nemotron` returned a bare `AUDIT COMPLETE - 17 findings` with no report body —
the reasoning-burn failure mode; treated as a **dead seat, not agreement**.

Every finding was verified against the real code before acting. Roughly half
did not hold up.

### Fixed (confirmed by ≥2 independent seats)

| # | Finding | Seats | Fix |
|---|---|---|---|
| 1 | `network_receive_buffer` ignored the broker's `status` field — a `{"status":500,"body":…}` response was handed to the CloudSync core as a **valid sync buffer**, and an error with a null body read as "no changes". | kimi (HIGH), mistral | Parse `status` immediately after the frame read; non-2xx → `CLOUDSYNC_NETWORK_ERROR`. |
| 2 | base64 decoder validated neither length-%4 nor the alphabet: a non-multiple-of-4 body made the last iteration decode the closing quote and trailing JSON into the payload, and any stray byte silently decoded as `0`. | gpt-oss, kimi | Reject `b64_len % 4 != 0` / empty, and validate every char against the alphabet (`=` only in the last two positions). |
| 3 | Whitespace skip after the `"body"` key handled only space/tab/colon, so a pretty-printed response was rejected as `bad body value`. | kimi | Skip `\n`/`\r` too (applied to the `status` scan as well). |

Re-verified after the fixes: two-node convergence proof still GO, `cargo test
-p sync-p2p` 2/2, `cargo test -p cloudsync --features from-source` 1/1.

### Rejected as false positives (verified against the code)

- **`base64_encode` overflows when `len % 3 == 1`** (mistral, HIGH) — false.
  `enc_len = ((len+2)/3)*4` is the standard ceiling and the allocation is
  `enc_len + 1`; `len=1` allocates 5, writes indices 0–3 plus NUL at 4.
- **base64 decode output buffer too small** (mistral, HIGH) — false. `dec_cap =
  (b64_len/4)*3 + 1` then allocates `dec_cap + 1`, i.e. one byte *more* than the
  maximum decode plus terminator.
- **`read_frame` ignores the `read_all` return** (mistral) — false. The exact
  check it proposes is already there (free + return NULL on short read).
- **Empty hostname not rejected before the copy** (mistral) — false; `host_len
  == 0` is checked on the line before the `memcpy`.
- **`json_escape` must escape `/`** (mistral) — false. JSON does not require it;
  the payload is a TCP frame to a Rust broker and is never embedded in HTML.
- **`json_escape` `strlen*6` integer overflow** (gpt-oss) — theoretically true,
  practically unreachable (needs a >3×10¹⁸-byte endpoint). Not actioned.
- **`read_frame` should reject `len == 0`** (gpt-oss) — benign; allocates 1 byte,
  `read_all` returns immediately, buffer is `""`.

### Carried to v0.6 as production requirements (real, out of spike scope)

- **⚠️ SSRF / no peer allowlist** (gpt-oss, HIGH). `endpoint` derives from the
  SQL-supplied `address`, so the extension will open a TCP connection to *any*
  host:port it is handed. In the spike that is the user's own localhost broker.
  In production the endpoint legitimately **is** a remote peer, so the fix is
  not "restrict to localhost" — it is the **device-pairing allowlist already in
  the v0.6 design**: refuse to connect to a node id / address that is not a
  paired peer. This finding independently validates that design requirement.
- **IPv6 endpoints are not usable** (mistral + gpt-oss, both HIGH). The
  last-colon split keeps the brackets, so `p2p://[::1]:1234/…` yields host
  `"[::1]"` and `getaddrinfo` fails. No memory-safety consequence (the host
  length is bounds-checked against `host_cap`) — it fails closed. The spike is
  IPv4-localhost by design; the production transport must strip brackets.
- **`strstr`-based field lookup is not JSON-aware** (kimi). `strstr(resp,
  "\"body\"")` and `strstr(resp, "\"ok\":true")` can match inside a *string
  value*. Doesn't fire against the current broker's response shapes, but the
  production transport should parse JSON properly (share jsmn from `network.c`
  or move the framing to a typed codec).
- **Non-UTF-8 bytes in the endpoint** (kimi) pass through `json_escape` and make
  serde reject the frame — fails closed as a sync error, no corruption.
- **No hostile-broker test fixture.** The three fixes above are defensive
  against responses the spike's own broker never produces, so the existing
  tests cannot regress them. v0.6 needs a fake-broker harness (non-2xx status,
  malformed base64, truncated frame) driving the extension end-to-end.