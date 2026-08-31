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

### Per-platform build outputs (linux/x86_64 proven in S0b; see §21 for SYNC-9)

| Target | Output | Status |
|---|---|---|
| linux/gnu/x86_64 | `cloudsync.so` | ✅ Proven (S0b): builds, loads, `cloudsync_version() == "1.0.12"` |
| linux/musl/x86_64 | `cloudsync.so` | `supported()` admits it (checks os+arch only, pre-dates SYNC-9) but it is untested; see §21 |
| linux/aarch64 | `cloudsync.so` | `supported()` admits it (SYNC-9) but **not locally verified** — no aarch64 toolchain on the dev box, no CI job either; see §21 |
| macos (aarch64/x86_64) | `cloudsync.dylib` | ✅ **Proven (SYNC-9, §21.11, CI run 33411261329):** `cargo check --features from-source` builds and links both `aarch64-apple-darwin` (native) and `x86_64-apple-darwin` (cross) on `macos-15`. Job still provisional/non-blocking pending a few more observed-green runs. |
| android (arm*/x86_64) | `cloudsync.so` | Still not built — needs `cacert.h`; out of scope, see §21 |
| windows/x86_64 | `cloudsync.dll` | `supported()` admits it (SYNC-9); Winsock2 port done in `network_p2p.c`; **not locally verified** — CI-only proof (§21, provisional job) |

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

---

## 13. SYNC-3 — iroh P2P transport + device identity + peer allowlist

Appended by SYNC-3 (the first v0.6 production increment over the S1 spike).
Replaces the spike's localhost TCP transport with a real iroh/QUIC P2P
transport, adds persistent device identity, and closes the §12 SSRF finding
with a peer allowlist enforced at both dial and accept. The wire format and
the broker control plane are unchanged from S1 — only the transport underneath
swapped, plus the identity/allowlist layer.

### 13.1 Dependency gate (GO)

iroh `1.1.0` added to `crates/sync-p2p`. The documented rustls-cascade risk did
**not** recur — iroh reuses the major versions the workspace already ships:

| crate | versions in lock after iroh | note |
|---|---|---|
| rustls | 0.22.4, 0.23.38 | unchanged — the same two majors that already coexisted (0.22 via libsql/hyper-rustls, 0.23 via AWS SDK **and now iroh**) |
| quinn | 0.11.9 | unchanged — iroh uses the same quinn major already present (a transitive reqwest dep) |
| ring | 0.17.14 | unchanged — ring was already in the lock pre-iroh |
| aws-lc-sys | 0.40.0 | unchanged |

iroh's default features use `tls-ring`; the app ships `aws-lc-rs`, but ring was
already present so no new crypto crate is introduced and no feature override
was needed. `cargo check -p sync-p2p --locked` and `cargo check -p desktop
--locked` both stay green.

### 13.2 The C↔iroh boundary: a local agent

The C `network_p2p.c` functions are synchronous and blocking (contract §6) and
cannot speak QUIC. The chosen design (brief-recommended): **the C layer stays
dumb and local; iroh lives entirely in Rust.**

- `network_p2p.c` opens a plain POSIX TCP socket to `127.0.0.1:<port>` — the
  in-process Rust **`P2pAgent`** (`crates/sync-p2p/src/agent.rs`) — and sends
  the **same** framed length-prefixed JSON the TCP spike used. The full
  endpoint URL (`p2p://<node-id>/...` or `mem://<node-id>/...`) travels inside
  the frame as the `endpoint` field; the C layer does not parse the node id.
- The agent's local TCP address is supplied by the host process via the
  `NOTARE_SYNC_AGENT_ADDR` env var (`127.0.0.1:<port>`), set when the agent
  starts. The C layer reads it per call (`getenv`).
- The `P2pAgent` owns the iroh `Endpoint` (secret key == device identity key),
  a `PeerStore`, and the local `BrokerState`. It routes each C request to the
  local broker (if the endpoint authority is this device's node id) or relays
  it to the addressed peer over an iroh bi-directional stream. `mem://` object
  URLs carry the *serving* peer's node-id fingerprint so `send_buffer` PUTs and
  download GETs route to the peer that minted them.

This quarantines the entire rustls/quinn/iroh dependency tree inside the Rust
process — exactly where the gate proved it coexists cleanly — and keeps the C
transport dead simple (one local socket, no crypto, no async runtime in the
extension). **C never speaks QUIC.**

### 13.3 Endpoint scheme

`cloudsync_network_init_custom(address, dbId)` builds endpoints as
`{address}/v2/cloudsync/databases/{dbId}/{siteId}/{action}`. The authority is
now a **node-id fingerprint** (`p2p://<compact-z-base-32-fingerprint>/...`)
rather than `host:port`, because iroh addresses a peer by its `EndpointId`
(Ed25519 public key), not by IP. The agent parses the fingerprint out of the
authority (`Fingerprint::parse`, which accepts both grouped-dashed and compact
forms) and dials that node id over iroh.

### 13.4 Device identity (`crates/sync-p2p/src/identity.rs`)

Persistent **Ed25519** keypair stored at `<data_dir>/notare/sync/device.key`
(0600 on unix, atomic write). The public key *is* the Device ID and *is* iroh's
`EndpointId` — iroh's `SecretKey` is itself Ed25519, so the same key is reused
directly (one identity, no second layer). Human-readable fingerprint: z-base-32
(iroh's native `PublicKey` encoding) grouped into dashed 4-char blocks
(Synthcing-style); round-trips through `Fingerprint::parse`.

### 13.5 Peer allowlist (`crates/sync-p2p/src/peers.rs`) — closes §12 SSRF

A `PeerStore` persisted as `<data_dir>/notare/sync/peers.json` — a **local
file, deliberately not a SQLite table and never registered with
`cloudsync_init`**. This is load-bearing: a CRDT-synced allowlist would let a
revoked device re-add itself by replicating its own still-present local row to
the revoking peer, undoing the revocation via the very sync it gates. A local
file outside the CRDT's reach makes revocation *structurally* irreversible from
a peer's perspective — only the local operator edits `peers.json`.

API: `add_peer` / `remove_peer` (revoke) / `list_peers` / `is_allowed(node_id)`
/ `touch_last_seen`. Node ids are stored as dashed fingerprints in the JSON
(human-auditable).

**Enforced at dial AND accept** (`agent.rs`):
- *Outbound* (`relay_request_to_peer` / `relay_put_to_peer`): before dialing a
  peer, `is_allowed(node_id)` is checked; a non-allowlisted node id → 403, no
  stream opened.
- *Inbound* (`accept_iroh`): the peer's `EndpointId` is authenticated by iroh
  during the TLS handshake (`conn.remote_id()` is verified, not self-asserted);
  a non-allowlisted peer → `conn.close(...)` immediately, no streams served.
  (The check is post-handshake because `Incoming` exposes only `remote_addr`
  pre-handshake, not the node id — but the node id is authenticated by the
  handshake, so the check is on verified identity.)

This is the production fix for the audit's §12 SSRF finding: rather than the
extension dialing any node id supplied via SQL, it dials only a paired,
allowlisted peer, and refuses unpaired inbound connections.

### 13.6 Broker refactor

`BrokerState::handle_request` / `handle_put` are now `pub(crate)` and reusable
over any transport (TCP or iroh bi-stream). The S1 `Broker` (localhost TCP
server) is a thin wrapper over them, so `tests/broker_protocol.rs` (which
drives the broker over raw TCP) stays green unchanged. The `mem://` URL label
(`addr` → `addr_label`) is now a routable string: `host:port` for the TCP
spike, node-id fingerprint for iroh.

### 13.7 Convergence proof (GO)

`cargo run -p sync-p2p --example sync_two_nodes` — two independent file-backed
SQLite DBs, two independent cloudsync site IDs, `cls` CRDT on
`notes(id INTEGER PRIMARY KEY, body TEXT)`, syncing over iroh (loopback,
`RelayMode::Disabled`). Node A hosts the shared broker; node B reaches it over
iroh. (CloudSync's protocol assumes a shared server both sites push to and
pull from; with iroh as the transport, B's connection to that shared broker is
a QUIC stream rather than a TCP socket. Full mesh — every device a broker — is
a later increment; same-machine convergence is the proof scope for this PR.)

A green run: A→B (2 rows over iroh), B→A (1 row), and a concurrent update on
row 1 converging conflict-free to the same value on both.

> The `sqlx-sqlite-worker` close panics at example exit (`(code: 5) unable to
> close due to unfinalized statements or unfinished backups`) are a pre-existing
> artifact (the cloudsync extension holds prepared statements sqlx's pool close
> cannot finalize) — the original S1 spike had the identical teardown. The
> convergence assertions all pass before close.
>
> ⚠️ **Harmless in the example; NOT harmless in SYNC-5.** In the example the
> process is exiting anyway, so a failed close costs nothing. Once a `P2pAgent`
> runs inside the desktop app, this becomes a close on the app's *real* database
> during shutdown — and Notare already has a bug class here (#101, recording
> data-loss because exit didn't drain before teardown). A close that fails with
> SQLITE_BUSY on exit risks leaving a hot journal/WAL behind. SYNC-5 must
> establish an explicit cloudsync teardown order — finalize/`cloudsync_terminate`
> before the sqlx pool closes — and assert it, rather than inheriting this.

### 13.8 What's plugged in where (for SYNC-4/5/6/7/8/9)

- **SYNC-4 (N-way / full mesh):** the proof uses a shared broker at A. Full
  P2P (each device runs a broker; peers dial each other directly) needs the
  broker's per-database log + per-site delivery to handle >2 peers, or a real
  global db_version. The `mem://` object store + control endpoints are already
  per-device; iroh's direct-connect/relay maps onto mesh naturally.
- **SYNC-5 (wire into the app):** start a `P2pAgent` per device in the desktop
  app, set `NOTARE_SYNC_AGENT_ADDR` for the cloudsync extension, and surface
  `Identity`/`PeerStore` through the Tauri command layer. Do NOT touch
  `crates/db-core`, `plugins/db`, or UI in this PR (constraint).
- **SYNC-6 (pairing UI):** `PeerStore::add_peer` is the API; the UI produces a
  node-id fingerprint (display via `Identity::fingerprint`, parse via
  `Fingerprint::parse`). Discovery is SYNC-8.
- **SYNC-7 (E2E payload encryption):** iroh already encrypts the transport
  (TLS over QUIC, peer-authenticated by `EndpointId`). Payload-level encryption
  (end-to-end across a relay, or at-rest) is layered on top.
- **SYNC-8 (rendezvous/relay):** the proof uses `RelayMode::Disabled` +
  `register_direct_addr` (a process-local node-id → socket-addr map). Production
  replaces that with iroh's relay/DNS/pkarr address lookup (`RelayMode::Default`,
  `EndpointAddr` discovered via the address-lookup service). NAT traversal is
  iroh's job — not hand-rolled.
- **SYNC-9 (cross-platform builds):** the C transport (`network_p2p.c`) uses
  POSIX sockets (linux). Windows/macOS need the equivalent local-socket setup
  + the `NOTARE_SYNC_AGENT_ADDR` handoff. iroh itself is cross-platform.

### 13.9 Audit carry-forward

The §12 production requirements not closed by identity/allowlist remain:
- **IPv6 endpoints** — the C layer's `resolve_agent_addr` splits at the last
  `:`; for IPv6 the agent address should be bracketed. The proof uses IPv4
  loopback; production should bracket/validate. (No memory-safety issue —
  bounds-checked.)
- **`strstr`-based JSON field lookup in `network_p2p.c`** — the response
  parsing still uses tolerant manual `strstr` for `status`/`body`. A hostile
  *agent* is not a threat (it's in-process, trusted), but SYNC-4's full mesh
  should move response parsing to a typed codec.
- **No hostile-peer test fixture over iroh** — the `iroh_transport.rs` tests
  cover allowlist refusal on dial + accept; a fixture driving malformed frames
  over a real iroh stream is SYNC-4.

---

## 14. Audit outcome (2026-08-28) — SYNC-3 (`agent.rs`)

`auditor` skill, seats `gpt-oss:120b` + `kimi-k2.7-code` (coder = glm-5.2, glm
family excluded), focused on the allowlist security boundary. Every finding was
verified against the real code before acting.

### Fixed

| Finding | Seats | Verdict + fix |
|---|---|---|
| **Inbound allowlist checked once per connection, not per bi-stream** — a peer revoked while its QUIC connection is open keeps being served until the connection drops. | kimi (medium) | **REAL but LATENT.** Verified: `dial_peer` currently opens a fresh connection per request, so the connection-level check alone already refuses a revoked peer — a test driven through the agent passes with *or without* the fix (confirmed by reverting it). The accept loop nevertheless serves `conn.accept_bi()` in a loop, so the gap goes live the moment SYNC-4 adds connection reuse. Fixed by re-checking per stream, and pinned by `revoked_peer_is_refused_on_a_reused_connection`, which opens two bi-streams on one connection and **fails against the pre-fix code**. |
| **Unparseable frame always answered with a `PutResponse`**, even when the caller sent a `Request`. | gpt-oss + kimi (2-seat) | **REAL**, low impact (only truly-garbage frames reach it, and the C side now fails closed on the §12 status check). Fixed: discriminate on `"url"`/`"blob"` and reply in the caller's shape. |
| **C-facing accept loop `break`s on any error** — a transient `EMFILE`/`ECONNABORTED` silently disables sync for the process lifetime. | gpt-oss (medium) | **REAL.** Fixed: log + back off 50ms + continue. (`tracing` added to the crate; it had no logging at all.) |
| **`endpoint_authority` also accepted `http://`** — a leftover from the S1 localhost spike. | kimi (low) | **REAL** (scheme confusion on attacker-influenced input). Fixed: `p2p://` only. |

### Carried to later PRs (real, out of scope here)

- **⚠️ SYNC-5: the C-facing localhost TCP port is unauthenticated.** Any local
  process can connect to the agent and read/write sync data, bypassing the peer
  allowlist from the local side. Wiring the agent into the desktop app is
  exactly when this must be closed — use a Unix-domain socket with peer-credential
  checks, or a token handed over alongside `NOTARE_SYNC_AGENT_ADDR`.
- **SYNC-4: unbounded per-stream task spawning.** Each inbound bi-stream spawns a
  task with no cap; an allowlisted-but-misbehaving peer could exhaust resources.
  Bound it with a semaphore when connection reuse lands.
- **SYNC-4: 64 MiB frame ceiling** is generous for a control-plane message; tighten
  once the real payload sizes are known.
- **SYNC-8: `lookup_direct_addrs` `try_lock` falls back to an empty address list**
  under contention, causing a spurious dial failure. This is proof-scaffold code
  (`register_direct_addr` exists only because `RelayMode::Disabled`); it is
  replaced wholesale by iroh relay/DNS discovery.

### Rejected

None outright this round — but note the *severity* of the per-stream finding was
overstated (reported as an active revocation bypass; it is latent until
connection reuse exists). Recording that distinction matters more than the label.

## 15. SYNC-4 — N-way convergence over an elected hub (GO)

`examples/sync_three_nodes.rs`, run with
`cargo run -p sync-p2p --example sync_three_nodes --features from-source`.
Three independent databases, three sites, one elected hub (node A), no server
anywhere. Proves the case two nodes structurally
cannot: with a single spoke the hub's delivery log is never more than one blob
ahead of anybody, so the interesting failure modes stay hidden.

Green run covers:

1. **hub -> both spokes.** A writes; B and C each receive it.
2. **spoke -> hub -> spoke.** B writes and C receives it, while B and C
   deliberately do **not** allowlist each other. No spoke-to-spoke connection
   exists, so this can only have travelled through the hub. This is the
   property that makes elected-hub a real topology rather than a two-party
   special case.
3. **multi-blob catch-up.** C writes twice while A and B are idle; both then
   drain *two* change sets each.
4. **multi-blob spoke-to-spoke.** B writes twice and C drains both, having
   never talked to B — step 2 at depth 1, step 3 at depth 2 into the hub, this
   is the combination.
5. **three-way concurrent update** on one row converging to a single value on
   all three, plus whole-table agreement across every row.

### 15.1 The finding: `check` serves ONE blob per call — callers must drain

The hub's `check` walks its append-only blob log from the calling site's
high-water mark and returns **the next unseen entry**, not all of them. One
`cloudsync_network_check_changes()` therefore advances a site by exactly one
change set.

Two nodes never expose this: the puller is only ever one blob behind, so a
single check looks complete. Add a second spoke and it is immediately wrong —
step 3 above needs two checks per node, and a caller that checks once silently
stays a change set behind, with no error and no signal.

**This is a SYNC-5 correctness requirement, not a nicety.** Whatever drives sync
in the app must loop until the hub reports nothing pending (`drain_check` in the
example is the reference shape: stop on `"rows":0` / a body-less 204, bounded so
a non-converging hub fails fast instead of spinning). Wiring the app to a single
check per tick would produce exactly the class of bug v0.6's gate exists to
catch: no crash, no error, just peers quietly diverging under load.

The alternative — have `check` return every pending blob in one response — was
not taken. One-blob-per-call keeps each response bounded, which matters once
blobs carry real session payloads and the transport is a relayed QUIC stream;
the drain loop is the cheaper place to absorb that.

### 15.2 What this does and does not establish

Established: the per-site high-water-mark design in `DbState.delivered` is
sound at N > 2, and the CRDT converges three ways. Elected-hub (plan topology A)
is proven.

**Not** established, and still open for the v0.6 gate:

- **Hub failover.** The hub is a single point of failure and the hub identity is
  static here. If A is offline, B and C cannot sync at all — they have no path
  to each other. Election, and what happens to the blob log when the hub
  changes, is unbuilt.
- **Blob-log growth.** The hub retains every blob forever. GC needs a
  "delivered to every *known* peer" watermark, which needs a peer roster the
  hub can trust — interacting with pairing (SYNC-6).
- Still same-machine, same-process, Linux/x86_64. Real two-desktop, offline
  reconnect and NAT-relay remain gate items.

### 15.2b Both proofs are behind `--features from-source` — deliberately

`sync-p2p`'s dev-dependency on `cloudsync` originally hard-enabled
`features = ["from-source"]`. **Cargo unifies features across the workspace**, so
that one line turned an opt-in, linux-only build into a workspace-wide one:
`cargo test --locked --workspace` on the macOS runner reached cloudsync's
`build.rs` and panicked with *"from-source is only implemented for linux/x86_64;
got macos-aarch64"*. It went unnoticed because the spike branch had never been
pushed — CI saw it the first time the branch was put through a PR.

The fix is feature wiring, not `build.rs`. cloudsync's prebuilt `include_bytes!`
artifacts are `cfg(not(feature = "from-source"))`, so the feature is an
either/or and there is nothing for `build.rs` to fall back *to* — a silent
return would only fail later and more confusingly. So: the dev-dependency no
longer forces the feature, `sync-p2p/from-source` maps to `cloudsync/from-source`,
and both examples declare `required-features = ["from-source"]` so a default
workspace build skips them instead of failing to compile.

Verified by resolved features rather than by "it built": a default
`cargo check --workspace --all-targets` reports `cloudsync: features=[]`.
**SYNC-9 must not undo this** — adding the other four targets means extending
`supported()` in `build.rs`, never making the feature default-on.

### 15.3 Audit (2026-08-28) — SYNC-4

`auditor`, coder=opus. ⚠️ **One of two seats was dead:** `nemotron-3-ultra`
returned `AUDIT COMPLETE - 7 findings` with an empty body — the reasoning-burn
failure mode, and the second time it has done this (see §12). It emits the
terminator line, so it passes the harness's liveness check while contributing
nothing. Treat a bare count with no findings as a dead seat, never as
agreement. That left an effective single-seat panel, so every finding was a
lead verified by hand rather than a corroborated fact.

**Fixed (verified real):**

- `rows_received` folded "unparseable reply" into "nothing pending", so a
  malformed or error reply ended the drain silently — precisely the
  silent-divergence class this file exists to warn about, and worse for being
  in code documented as the shape SYNC-5 should copy. Now returns `Option` and
  the caller fails loudly on `None`.
- `drain_check` fell out of its `MAX_DRAIN` bound and returned normally, so
  hitting the bound was indistinguishable from a drained queue. Now panics.
- The convergence loop exited on the first round where all three agreed. Three
  sites can match on an intermediate value while the hub still holds a blob
  that would move one of them, so a pass did not strictly imply convergence.
  Now runs a settling round and requires the value to be **unchanged** —
  "agreed AND stable" rather than a moment that happened to line up.
- Added scenario 4 (multi-blob spoke-to-spoke), which was the one delivery
  combination the proof left uncovered.

**Rejected as a false positive:** a claim that `drain_check` should reconcile
the hub's reported row count against rows actually applied, to catch partial
delivery. `rows` is not a hub promise — it is the core reporting what it
applied, so there is no gap to check. The proposed fix also compared a table
total against a sum of changeset row counts, which are different quantities
(updates change no total) and would have produced false panics on any update.

**Noted, not changed:** the `unsafe { set_var(NOTARE_SYNC_AGENT_ADDR) }` per-node
switching is brittle. It is sound here because the example is strictly
sequential, but a process-global env var as the C layer's routing channel does
not survive more than one agent per process. Not a problem for the desktop app
(one agent), but SYNC-5 should not widen it. The seat's proposed fix invented a
`cloudsync_network_sync_custom` SQL function that does not exist.

## 16. SYNC-5 — wiring the sync stack into the app (2026-08-30)

Scope: `plugins/db` (sync lifecycle + commands), desktop wiring, the
`hypr_db_app` table registry, and the three fixes the new lifecycle test
forced. Four commits on `feat/sync-5-wire-plugins-db`:
`df3c6b3e1` (plugins/db sync module), `cc1b450e9` (desktop wiring),
`528a8e085` (SYNCED_TABLES), `6d743fab5` (lifecycle-test findings + test).

### 16.1 What was built

- `plugins/db/src/sync.rs` — `SyncLifecycle`: agent up → publish
  `NOTARE_SYNC_AGENT_ADDR`/`NOTARE_SYNC_TOKEN` (single point of publication)
  → `cloudsync_configure` (own `p2p://<fingerprint>` as the address, elected
  hub, table registry from `hypr_db_app`) → `cloudsync_start`. Teardown split
  so `PluginDbRuntime::shutdown` can run the #101 order:
  `cloudsync_stop` → live-query dispatcher stop → `pool().close()` → agent.
- `plugins/db/src/runtime.rs` — `PluginDbRuntime::shutdown(&self)` (the Arc
  stays in Tauri's state map; the dispatcher stops through an explicit
  `LiveQueryRuntime::shutdown`), plus `start_sync` / `sync_status` /
  `sync_trigger` / `sync_list_peers` / `sync_this_device`, all behind
  `all(feature = "sync", target_os = "linux")`.
- `plugins/db/src/commands.rs` — four specta commands with cfg-gated bodies
  (unconditional names: `collect_commands!` rejects `#[cfg]` entries, so the
  bindings surface stays identical across feature sets).
- `apps/desktop/src-tauri/src/lib.rs` — `start_sync` in setup (spawned,
  best-effort), `shutdown_sync` on `RunEvent::Exit` on a dedicated thread with
  its own current-thread runtime and a 5 s bound.
- `crates/db-app/src/cloudsync.rs` — the table registry now carries
  `enabled: SYNCED_TABLES.contains(&table_name)`.

### 16.2 Three production bugs the lifecycle test caught

Each was invisible to `cargo check` and to every earlier test, because none of
them had ever driven the full stack in one process.

1. **1-arg `cloudsync_network_init(dbId)` ignored the configured address.**
   The C layer hardcodes `CLOUDSYNC_DEFAULT_ADDRESS`
   (`https://cloudsync.sqlite.ai`) in that form; only the 2-arg
   `cloudsync_network_init_custom(address, dbId)` routes `p2p://` addresses.
   Symptom: `network_init` "succeeds" against a URL that has nothing to do
   with our agent. Fixed in `crates/cloudsync/src/network.rs`.
2. **Per-connection auxdata vs a 4-connection pool.**
   `dbsync_register_functions` installs a fresh `cloudsync_context` per
   sqlite3 handle; network_data lives in per-connection auxdata, so with
   more than one pooled connection a network call can land on a connection
   whose auxdata was never initialized → "Unable to retrieve CloudSync network
   context". `Db::open` now clamps cloudsync-enabled opens to
   `max_connections(1)` (`crates/db-core/src/lib.rs`).
3. **Runtime `std::env::var` vs compile-time `env!` for
   `CLOUDSYNC_FROM_SOURCE_SO`.** Cargo injects a build-script's `rustc-env`
   only into that package's own binaries — sync-p2p's build.rs sets it for
   sync-p2p's own test/example binaries, but not for tauri-plugin-db's. The
   bundle path must be baked with `env!` at compile time
   (`crates/cloudsync/src/bundle.rs`).
4. **Zero-enabled-tables fatal first tick.** (Found by reasoning, not the
   test, but in the same session.) With no `cloudsync_init(<table>)` anywhere,
   the `cloudsync_changes` shadow table does not exist and every send/check
   query fails fatally (SQLite code 1), killing the background loop's first
   tick. `cloudsync_start`/`cloudsync_trigger_sync`/`cloudsync_status` now
   short-circuit to an honest no-op when `enabled_tables().next().is_none()`
   (`crates/db-core/src/cloudsync/runtime.rs`).

### 16.3 Tables enabled: none (a STOP, per spec)

`SYNCED_TABLES = []` in `crates/db-app/src/cloudsync.rs`, deliberately. The
spec's instruction was explicit: justify any enable from what
`crates/sync-p2p/examples/sync_three_nodes.rs` actually converged on, and
**stop and report rather than enabling broadly** if the mapping is unclear.

The proofs (sync_two_nodes, sync_three_nodes, drain_regression) converge a
**synthetic** `notes (id INTEGER PRIMARY KEY, body TEXT)` table — a minimal
INTEGER PRIMARY KEY rowid table. Notare's app tables are UUID-text-primary-key
tables (sessions, session_tags, notes etc.) with FKs between them
(sessions → meeting FK, etc.). The proofs say nothing about:

- whether the vendored cloudsync CRDT handles TEXT primary keys at all,
- FK cascading under CRDT merge order,
- per-table `crdt_algo` choice for notare's actual mutation patterns,
- or which subset of the schema forms a safe unit to sync first.

Enabling app tables on that evidence would have been guessing. SYNC-6 (pairing
UI) is where the enable-set is decided, with a proof against the real schema.
This is the spec's STOP, documented, not a silent omission.

### 16.4 Audit outcome (2026-08-30) — the SYNC-5 commits

`--coder glm-5.3`, `--scope dfb57baf1` (all four commits), split by area with
`--only`. Seats: `audit-minimax-m2.7` (plugins/db area, 7 findings) +
`audit-qwen3.5:397b` (second seat; the roster's `kimi-k3` is not routable on
the gateway — see below). Payloads kept under the ~20k-char size cliff by
`--only` per area.

**Confirmed and fixed (verified real):**

- **`sync_this_device` used `blocking_lock()` in a sync fn** (and
  `sync_list_peers` a try_lock+fallback). The command is async;
  `start_sync`/`shutdown` hold the runtime's sync mutex across awaits
  (network init can take seconds), so a blocking acquire stalls an executor
  thread — and deadlocks outright on a current_thread runtime, the exact
  pattern the shutdown path uses. Both methods are now async with
  `.lock().await` and one shared fallback. This also removed the duplicated
  PeerStore fallback the same seat flagged separately (its Finding 4).
- **SAFETY comment on `set_var` understated its contract** (seat's Findings 2
  + 6). The comment now states the actual invariant: exactly one
  `SyncLifecycle` per process; in the app structural (one
  `PluginDbRuntime`, mutex-held startup, readers spawn only after
  publication inside `cloudsync_start`); direct `start_with` callers (tests)
  must serialize.

**Rejected as a false positive (verified against the code):**

- **CRITICAL claim that `guard.take()` makes `cloudsync_stop` never run**
  (minimax seat Finding 1). `take()` *returns* the owned
  `Option<SyncLifecycle>` — `lifecycle.as_ref()` borrows what `take()`
  returned, and step 4 moves it into `stop_agent`. The seat read `take()` as
  "discard". Nevertheless pinned with a regression test
  (`runtime_teardown_runs_every_step_and_fallbacks_answer_without_lifecycle`)
  because the #101-critical step deserves a direct guard, and that test also
  covers the no-lifecycle fallbacks on `sync_this_device`/`sync_list_peers`.
- **"start_sync idempotency vs bare-db fallback is undocumented"** (Finding
  5, LOW). Deliberate contract: before `start_sync`, status reports the
  honest bare-db state; the desktop always calls `start_sync` at setup.
  No change.
- Finding 7 was the seat itself saying "no action needed" (test-drop
  ordering). Agreed.

**Operational note:** the audit lane's roster names `kimi-k3`, but the gateway
has no `audit-kimi-k3` group — preflight correctly refused it, and the second
seat ran on `audit-qwen3.5:397b` via `--models` instead. The roster probe
records what the provider advertises, not what the gateway can route; when a
seat is refused, re-run the missing seats with an explicit routable model
before treating the audit as done.

### 16.5 Desktop-area audit (the audit's biggest catch)

The desktop-area run (`--only apps/desktop`, both seats) found the defect
every other check had missed:

**Confirmed and fixed (qwen Finding 1, verified with `cargo tree`):** the
target-gated `tauri-plugin-db = { features = ["sync"] }` entry in the
desktop crate was non-optional, so on linux/x86_64 cargo unified it with the
plain dep in `[dependencies]` and pushed `sync` into EVERY desktop build —
`cargo check -p desktop` "default" was silently the sync-on config, and
`--features sync` was a no-op. The spec's default-OFF invariant was broken on
exactly the machine the checks ran on, which is why the earlier verification
looked green while proving nothing. Fix: the target-gated dep is now
`optional = true` and the `sync` feature activates it via
`sync = ["dep:tauri-plugin-db", "tauri-plugin-db/sync"]`. Verified after:
default resolves `tauri-plugin-db` with no `sync` feature;
`--features sync` resolves the full stack. The `cargo tree -p desktop -e
features -i tauri-plugin-db` inversion is the honest way to check this —
`cargo check` cannot see it.

**Confirmed and fixed (both seats agreed — high confidence):** my
`cc1b450e9` had accidentally duplicated the
`ctx.mark_exiting(); ctx.stop();` supervisor-stop block in `RunEvent::Exit`.
Copy-paste from the insertion; removed.

**Confirmed and applied (qwen Findings 2/3):** `start_sync` and
`shutdown_sync` used `app.state::<ManagedState>()`, which panics when the
state is absent — contradicting the "best-effort, keep running" contract on
start and the "must exit, never panic on the way out" contract on Exit. Both
now use `try_state()` with graceful degradation.

**Confirmed and applied (qwen Finding 6):** the desktop/plugin source gates
said `target_os = "linux"` while the `sync-p2p` dep only exists on
`linux/x86_64`. Aligned every `cfg(all(feature = "sync", target_os =
"linux"))` to include `target_arch = "x86_64"` (desktop lib.rs, plugin
lib.rs/runtime.rs/commands.rs, the lifecycle test). On aarch64-linux +
`--features sync` the failure is now "feature exists but gates nothing",
not a compile error in the plugin.

**Rejected / no change (with reasons):**

- qwen Findings 2/3 from the cloudsync area ("fetch_optional silently
  swallows init errors") — false positive, proven from the vendored C:
  every failure path in `cloudsync_network_init_internal` (and
  `set_token`/`set_apikey`) signals via `sqlite3_result_error*`, which makes
  the whole SQL statement fail; sqlx propagates that as `Err` and
  `.await?` surfaces it. The C layer has no out-band "logical error inside
  a success row" channel to swallow.
- minimax's "pre-existing SQL syntax error in query_sync_json" — the four
  arms are `SELECT fn(...) AS it`, all well-formed; the seat showed the
  "corrected" text as identical to what's there. Also the mainline path of
  the green drain regression test.
- minimax's `CLOUDSYNC_MANAGED_DB_ID` cross-deployment collision note —
  real observation, out of scope by design: the fixed id matches the proofs'
  DB_ID so the same hub recognises every device of the app; per-deployment
  ids are a SYNC-8 (relay/rendezvous) design decision.
- qwen Finding 4 + minimax's shutdown-thread notes (double timeout,
  join/catch_unwind, 5s vs 6s bounds) — design suggestions on correct,
  bounded behavior. The outer `recv_timeout(SHUTDOWN_TIMEOUT + 1s)` is a
  deliberate belt-and-suspenders bound in case the inner timeout itself
  never fires. A panicking teardown thread is indistinguishable from a
  wedged one by design: both log a warning and the process exits anyway.
- minimax's "best-effort start_sync failures are invisible to the user" —
  true and accepted for SYNC-5; surfacing sync status to the UI is SYNC-6
  (the `sync_status` command exists precisely for that).

**db-app area:** clean (0 findings; single seat on a one-constant diff).

**Remaining lead from §15 carried forward, not re-found:** the §15 note about
per-node env-var switching in the examples remains the standing constraint:
one agent per process. SYNC-5 did not widen it — `SyncLifecycle` is the single
publisher, and the audit confirmed the contract now states it.

## 17. SYNC-6 part A — real-schema convergence proof (2026-08-30) — GO

SYNC-5 wired the stack into the app but enabled **zero** tables
(`SYNCED_TABLES = []`) because §11–§15 only proved convergence on a synthetic
`notes (id INTEGER PRIMARY KEY, body TEXT)` table. This section closes that gap:
`crates/sync-p2p/examples/sync_sessions_schema.rs` (from-source gated) converges
the **real** session schema — `sessions` and `session_documents` — which are
TEXT-PK, `STRICT`, carry NOT-NULL-defaulted TEXT columns, a `deleted_at`
tombstone column, and a FOREIGN KEY (`session_documents.session_id → sessions.id`).

Run: `cargo run -p sync-p2p --example sync_sessions_schema --features from-source`.

**All scenarios PASS over iroh with `cls`:**
- A→B and B→A converge — TEXT-PK rows, STRICT typing, FK intact.
- Disconnected concurrent UPDATE of the same row converges conflict-free
  (single value on both nodes, equal to one of the writes — no torn merge).
- **Tombstone-as-delete:** A sets `deleted_at` on a session and hard-deletes its
  child document; B drains — the `deleted_at` value syncs, the child is gone,
  and the row does **not** resurrect across further sync rounds. This is the
  v0.6-gate property the trash view depends on.
- Multi-row catch-up: several inserts on both sides drained to full set
  equality (exercises the SYNC-5 drain loop).

**What this unblocks:** `sessions` + `session_documents` are now *proven* to
converge and are candidates for SYNC-6's `SYNCED_TABLES` enable-set. The
remaining 15 registered tables (transcripts, tags, action_items, …) are NOT yet
proven — enable each only after it gets the same proof, especially any with
different PK shape or a self/multi-level FK.

**Known noise:** the example exits with the `sqlx-sqlite-worker` "unable to
close due to unfinalized statements" panic — the same benign teardown ordering
§13.7/§16 recorded. SYNC-5's app teardown orders cloudsync_stop before pool
close, so this is example-lifecycle noise, not an app defect.

## 18. SYNC-7 — E2E payload encryption (2026-08-31)

### 18.1 What was built

Added per-peer payload encryption in `crates/sync-p2p` at the iroh/QUIC peer
boundary only. The local C↔agent TCP hop (gated by SYNC-5's bearer token) stays
plaintext; the frames that leave the device over iroh are now encrypted.

Files changed:
- `crates/sync-p2p/src/crypto.rs` — new module with key derivation, XChaCha20-Poly1305
  encrypt/decrypt, and unit tests.
- `crates/sync-p2p/src/agent.rs` — wired encryption into outbound relay
  (`relay_request_to_peer`, `relay_put_to_peer`) and inbound `serve_peer_stream`.
- `crates/sync-p2p/Cargo.toml` — added `chacha20poly1305`, `hkdf`, `sha2`, `rand`,
  `ed25519-dalek` (direct, was already a transitive dep via iroh).
- `crates/sync-p2p/src/lib.rs` — exported the `crypto` module.
- `Cargo.lock` — resolved new crates; no duplicate rustls/crypto stacks.

### 18.2 Key-derivation recipe

1. Ed25519 identity reuse: each device already has an iroh `SecretKey` / `PublicKey`
   (Ed25519). Convert to X25519 static keys using `ed25519-dalek` helpers:
   - secret scalar: `SigningKey::to_scalar_bytes()`
   - public Montgomery u: `VerifyingKey::to_montgomery().0`
2. X25519 DH: `MontgomeryPoint(peer_montgomery_u).mul_clamped(my_scalar_bytes)`
   via `curve25519-dalek`.
3. HKDF-SHA256 over the raw 32-byte shared secret, info string:
   `b"notare-sync-p2p-v1:" || sorted(id_a, id_b)` (lexicographic, raw 32-byte
   public keys). Output is a 32-byte symmetric key.
4. XChaCha20-Poly1305: random 24-byte nonce per message, prepended to the
   AEAD ciphertext (`[nonce || ciphertext+tag]`). The nonce comes from `rand::fill`
   (the same CSPRNG source already used for the SYNC-5 token).

### 18.3 Invariants verified by tests

- `crypto::tests::round_trip_encrypt_decrypt` — A encrypts for B, B decrypts,
  plaintext identical.
- `crypto::tests::tampered_ciphertext_fails` — bit flip in ciphertext is rejected.
- `crypto::tests::key_is_bound_to_peer_pair` — A↔B key ≠ A↔C key, and C cannot
  open an A→B message.
- `crypto::tests::wrong_sender_public_key_fails` — receiver using the wrong
  sender public key fails to decrypt.
- The existing integration tests (`iroh_transport.rs`, `broker_protocol.rs`) and
  convergence examples (`sync_two_nodes`, `sync_three_nodes`) pass with encryption
  enabled unconditionally.

### 18.4 Audit outcome (2026-08-31)

Ran the repo `auditor` skill on the SYNC-7 commits with `--coder kimi-k2.7-code`
(the model that wrote the code). Findings:

- **No confirmed crypto defects.** Key derivation uses the crate-provided
  Ed25519→X25519 conversion, HKDF domain separation binds both sorted node ids,
  nonces are 24 random bytes per message, and AEAD failures return hard framed
  errors before the broker sees data.
- **No new warnings** from `cargo check -p sync-p2p --all-targets`.
- **No duplicate rustls/crypto stacks** introduced: `cargo tree -p sync-p2p`
  shows the only new standalone crypto crates are `chacha20poly1305`, `hkdf`,
  and `sha2` (direct), plus `ed25519-dalek` made direct (it was already pulled
  by iroh). `curve25519-dalek` remains a transitive dep of iroh/ed25519-dalek.

All verification items green:
- `cargo check -p sync-p2p --all-targets` — clean.
- `cargo test -p sync-p2p` — all 25 tests pass.
- `cargo test -p cloudsync --features from-source` — passes.
- `cargo run -p sync-p2p --example sync_two_nodes --features from-source` — converges.
- `cargo run -p sync-p2p --example sync_three_nodes --features from-source` — converges.
- `pnpm exec dprint fmt` — touched files formatted (Swift formatter missing on
  this Linux box is pre-existing and unrelated).
**Audit (2026-08-31, 2 seats, coder kimi-k2.7-code):** crypto.rs and agent.rs audited separately
(the combined payload overflowed the seat argv limit — the auditor warns and you must split with
`--only <path>`). crypto.rs: gpt-oss found nothing; minimax's one finding is real-but-low —
`rand::fill` panics if the OS CSPRNG read fails instead of returning `CryptoError::Encrypt`. Not
fixed this round: the workspace is pinned to rand 0.7 (via iroh) which has no `try_fill`, and the
failure only occurs under an OS entropy outage; carried forward to the next rand/iroh bump.
agent.rs: gpt-oss's "critical duplicate helper definitions" and minimax's "non-compiling
intermediate" are FALSE POSITIVES — both read a two-hunk diff as if the intermediate state were
shippable. Verified against the committed tree: each helper is defined exactly once and
`cargo check -p sync-p2p --all-targets --features from-source` compiles clean.

## 19. SYNC-6 part B — enable-set + device pairing (2026-08-31)

This section was numbered around §18 (SYNC-7, E2E payload encryption), which
landed from its own branch in parallel. The numbering avoids a semantic
collision, not a textual one — both branches appended at end-of-file, so the
merge did conflict here and was resolved by keeping both sections in order.

**Enable-set.** `SYNCED_TABLES` in `crates/db-app/src/cloudsync.rs` goes from
`[]` to exactly `["sessions", "session_documents"]` — the two tables §17 proved
converge. The other 15 registered tables stay `enabled: false`; each needs its
own §17-style proof before it is added.

**Pairing surface.** `SyncLifecycle::add_peer` / `remove_peer`
(`plugins/db/src/sync.rs`) back two new specta commands, `sync_add_peer` and
`sync_remove_peer`. Both are reachable whether or not the lifecycle is running:
the allowlist is a local JSON file, so `PluginDbRuntime` opens the `PeerStore`
directly on the not-started path (`runtime.rs`), holding the `sync` mutex across
the whole call so concurrent pairing serializes. The allowlist stays local and
un-synced — that invariant is unchanged.

**Fingerprint shape.** `this_device()` returned compact z-base-32 from the
lifecycle path and the grouped/dashed form from the not-started fallback. Both
now return the grouped form, since it is what the UI displays and
`Fingerprint::parse` round-trips either. `SyncPeer` gained a `fingerprint`
field (grouped, for display) alongside `node_id` (compact, canonical — still
what `sync_remove_peer` takes).

**Defect found and fixed: the SYNC-5 sync commands were unreachable.** §16
registered `sync_status` / `sync_trigger` / `sync_list_peers` /
`sync_this_device` with specta but never added them to `plugins/db/build.rs`
`COMMANDS` or `permissions/default.toml`. The desktop app grants only
`db:default`, so every one of them would have been denied by Tauri's capability
layer the first time the frontend called it — invisible until a UI existed. All
six commands (the four above plus the two new ones) are now registered and
permitted. This is a SYNC-5 omission, not a SYNC-6 regression.

**Discrepancy in the §17 proof, recorded not fixed.** The proof example
`sync_sessions_schema.rs:72` declares `FOREIGN KEY (session_id) REFERENCES
sessions (id)` on its `session_documents` replica, and §17 describes the real
schema as carrying that FK. The actual migration
(`crates/db-app/migrations/20260710223922_canonical_data_model.sql:59`) declares
**no** foreign key on `session_documents` — nor does any table in the registry.
The proof therefore ran against a *stricter* schema than production. That is the
safe direction (convergence under an enforced FK implies convergence without
one), so the enable decision stands, but §17's "FK intact" wording overstates
what production enforces. A later proof should either drop the FK to match the
migration, or the migration should gain one deliberately.

**Audit outcome** (`--coder claude-sonnet-5`, per-commit panels plus a
consolidated pass over the fix commit). Confirmed and fixed: the add-device form
validated the normalized fingerprint but submitted the raw one, so a grouped or
mixed-case paste could pass client validation and then fail the z-base-32 decode
in `Fingerprint::parse`; the copy-to-clipboard `setTimeout` was cleared neither
on unmount nor on repeat clicks. Refuted against the real code: a claimed
dangling-FK / cascade hazard from enabling `session_documents` (no FK exists —
see above); a claimed race in the not-started `add_peer` path (the mutex guard
covers the whole body); a claimed `PublicKey`/`&str` type mismatch in
`PeerStore::add_peer` (it takes `PublicKey`); a claimed cfg-gate asymmetry
between the two new commands (the gates are identical). The observation that the
pairing commands land in the plugin's single flat `default` permission set is
accurate but not a regression — every command in this plugin, including raw
`execute`, has always done so; splitting the permission model is its own piece
of work.

## 21. SYNC-9 — cross-platform `from-source` builds

S0b through SYNC-6 proved the sync stack end-to-end, but `crates/cloudsync`'s
`from-source` build only ever compiled on linux/x86_64 — `build.rs` panicked
on every other target. SYNC-9 widens that, and ports the custom C transport
(`build/network_p2p.c`) off POSIX-only sockets so Windows can build it. It
does **not** touch the protocol, the agent, or the peer allowlist — this
section is entirely about "does the C code compile and link," not "does sync
work" (that was already proven on linux/x86_64 in §12/§13/§15).

### 21.1 `supported()` — what's admitted now

`crates/cloudsync/build.rs`'s `supported()` went from a single `os ==
"linux" && arch == "x86_64"` check to a `matches!` over five target pairs:

```rust
matches!(
    (os.as_str(), arch.as_str()),
    ("linux", "x86_64")
        | ("linux", "aarch64")
        | ("macos", "aarch64")
        | ("macos", "x86_64")
        | ("windows", "x86_64")
)
```

Android stays out of `supported()` — it needs
`vendor/src/network/cacert.h` (an Android-only PEM bundle wired up
separately), unrelated to this lane. The panic message on an unsupported
target names it explicitly. **The feature stays default-OFF and opt-in** — no
line in this change enables `from-source` anywhere by default (§15.2b's
constraint); a default `cargo check -p cloudsync -v` still shows no
`--cfg feature="..."` line for the crate, i.e. `features=[]`.

One pre-existing wrinkle, not introduced by this change: `supported()` checks
`target_os`/`target_arch` only, not `target_env`, so `x86_64-unknown-linux-musl`
also satisfies `("linux", "x86_64")` — it did before SYNC-9 too, since the
original check had the same shape. Nothing here makes that better or worse;
it is untested either way (no musl target on the dev box, no CI job for it).

### 21.2 The link step is now genuinely platform-specific

Compiling the vendored C sources to objects was already portable (`cc::Build`
translates `.define()`/`.opt_level()`/`.warnings()` per compiler); the two
places that were NOT portable:

- `-fPIC`: meaningless on Windows and rejected outright by `cl.exe`. Now
  added only when `target_os != "windows"`.
- The final manual link (`cc::Build::compile()` is deliberately not used here
  — see the existing comment — because it would emit `cargo:rustc-link-lib`
  and wrongly link the extension into the host binary). This now branches
  three ways:
  - **linux**: unchanged, `-shared -o cloudsync.so <objects> -lm`.
  - **macOS**: `-dynamiclib` instead of `-shared` — chosen explicitly to
    produce a real `.dylib` (matching the `.dylib` extension the prebuilt
    `vendor/cloudsync/macos/` artifacts already use) rather than relying on
    clang's Darwin driver to alias `-shared`. This part was right the first
    time — **but the link still failed on real CI** (§21.10) for an unrelated
    reason: `-framework Security` was missing. `vendor/src/utils.c` calls
    `SecRandomCopyBytes`/`kSecRandomDefault` (`<Security/Security.h>`) for
    UUID generation on Apple targets, and neither `-dynamiclib` nor anything
    else in the link command pulled that framework in, so `arm64` came back
    `Undefined symbols ... ld: symbol(s) not found`. Fixed by adding
    `-framework Security` on the macOS branch only (linux/Windows untouched).
    `-framework CoreFoundation` was considered and **not** added — nothing in
    `vendor/src` or `build/network_p2p.c` calls a CF*/CoreFoundation symbol
    (checked by `grep`), and Security.framework resolves its own internal
    CoreFoundation dependency without our object files needing to reference
    it directly. This is inference, not an observed link — the next CI run
    is the actual proof (§21.10).
  - **Windows (MSVC, `compiler.is_like_msvc()`)**: `cl.exe` does not
    understand `-shared`/`-o`. `cl /LD /Fe:<path> <objects> ws2_32.lib` tells
    `cl` to produce a DLL and invoke `link.exe` itself; the `.o` object files
    `cc::Build::compile_intermediates()` produces are passed straight to
    `link.exe` by `cl`'s documented "unrecognized extension → passed to the
    linker" behavior. **Still entirely unverified** — the first real CI run
    never reached this code at all; it died inside a shared setup action
    before `cargo` ever ran (§21.10). This remains documented behavior, not
    observed behavior.
  - Windows/GNU (mingw-w64) falls through to the `-shared` branch plus
    `-lws2_32`, kept working for anyone building outside this repo's CI, but
    not what the CI job below uses (it targets `x86_64-pc-windows-msvc`, same
    as the rest of this repo's Windows builds).

The output filename also now varies: `cloudsync.so` / `cloudsync.dylib` /
`cloudsync.dll`, matching what `bundle.rs`'s prebuilt path already names them
(the from-source path doesn't care about the extension at runtime — it loads
whatever path `cargo:rustc-env=CLOUDSYNC_FROM_SOURCE_SO` points at — but
matching the convention avoids a confusing `.so` on Windows in a debugger).

### 21.3 `network_p2p.c` — Winsock2 port

Ported behind a single `#ifdef _WIN32` block near the top of the file (protocol
logic — framing, JSON building, base64 — is untouched):

- `sock_t` (typedef `SOCKET` on Windows, `int` elsewhere), `SOCK_INVALID`
  (`INVALID_SOCKET` vs `-1`), `CLOSESOCK()` (`closesocket` vs `close`) replace
  every raw `int fd` / `close(fd)` in the file.
- `IS_RETRYABLE_SEND_RECV_ERROR()` — `errno == EINTR` on POSIX, `false` on
  Windows (Winsock blocking sockets have no EINTR-equivalent retry case).
- One-time `WSAStartup`/`WSACleanup`: a static `bool` guard calls
  `WSAStartup` on first use and registers `WSACleanup` via `atexit`. Not
  thread-safe against a concurrent first call from two threads — same
  simplicity tradeoff as the rest of this file (contract §6: the core calls
  these functions synchronously, one at a time).
- `send`/`recv` calls now cast `len` to `int` and the return to `int`
  uniformly on both platforms (Windows' signatures require `int`; every call
  site is bounded well under `INT_MAX` by the 64 MiB frame cap in
  `read_frame`, so the cast is exact, not lossy, on POSIX too).
- `connect()`'s `addrlen` argument is cast to `int` rather than `socklen_t`,
  since Winsock's `connect()` takes a plain `int` and some Windows SDKs don't
  define `socklen_t` at all; POSIX's `socklen_t` parameter accepts an `int`
  argument without complaint.
- `build.rs` links `ws2_32.lib`/`-lws2_32` on Windows (§21.2).

**macOS** needed none of *this* — BSD sockets, and every header
`network_p2p.c` itself includes (`<sys/socket.h>`, `<netinet/in.h>`,
`<arpa/inet.h>`, `<netdb.h>`, `<unistd.h>`) ships with Xcode's SDK; that part
held up on real CI. What did **not** hold up was the assumption that macOS
needed nothing *at all* to link successfully — the real failure was one
directory over, in the pre-existing vendored `utils.c` (§21.2, §21.10), which
this lane did not audit because it isn't `network_p2p.c`. The lesson: "this
file's headers are all fine" is not the same claim as "the whole from-source
build links," and only the second one is what CI actually tests.

### 21.4 The `strstr` JSON parsing — still open, untouched here

Per the spec for this lane, the `strstr`-based response field lookup in
`network_receive_buffer`/`network_send_buffer` (`"status"`/`"body"`/`"ok"`,
carried since §12/§13.9) was **not** touched. It is orthogonal to
cross-platform support — the same tolerant parsing runs on every target — and
mixing it into this change would blur two unrelated diffs. Still open, still
tracked as production hardening for a typed codec / hostile-broker fixture.

### 21.5 IPv6 bracket fix (closes the §12/§13.9 carried finding)

`resolve_agent_addr` splits `NOTARE_SYNC_AGENT_ADDR` at the last `:`, so a
bracketed IPv6 literal like `[::1]:1234` produced host `"[::1]"` — brackets
included — which `getaddrinfo()` rejects (they're a URI/host:port display
convention, not part of a valid numeric host argument). Fixed by stripping a
matching leading `[` / trailing `]` from the host substring immediately after
the split, before it's used anywhere. The `host_cap` bounds check (fails
closed on an oversized host) and the "`NOTARE_SYNC_AGENT_ADDR` not set"
failure path are both unchanged in shape and behavior.

The fix also became the occasion to extract `resolve_agent_addr` (and the new
`resolve_agent_addr_from`, the pure version that takes the address as a
parameter instead of reading the env var) into `crates/cloudsync/build/agent_addr.h`
— a small, dependency-free header (`getenv`/`memcpy`/`strtol` only, no
cloudsync allocator, no sockets) shared by `network_p2p.c` and by a new test,
`crates/cloudsync/tests/ipv6_bracket_host_split.rs`. That test compiles a
tiny, self-contained C harness against `agent_addr.h` and runs it as a
subprocess (five cases: bracketed IPv6, plain IPv4 unaffected, "not set"/"no
colon" fail closed, an empty bracketed host `"[]:port"` fails closed (§21.9),
and the `host_cap` bounds check still fails closed on an oversized bracketed
host) — gated `#[cfg(feature = "from-source")]`, run by `cargo test -p
cloudsync --features from-source`.

### 21.6 CI coverage (Requirement 4)

`from-source` is default-OFF, so before this change **no CI job had ever
compiled this C code anywhere but linux/x86_64** — widening `supported()`
without CI coverage would only have moved an untested claim from the panic
message into the docs. `.github/workflows/desktop_ci.yaml` gained two new
jobs, both scoped to `cargo check -p cloudsync --features from-source` only
(no other job in the file enables the feature, and neither job does a full
app build — `cargo check` alone is enough because Cargo runs build scripts
for `check`, not just `build`, so it still drives the real C compile-and-link):

- **`cloudsync_from_source_macos`** (`macos-15`, Apple Silicon): checks both
  macOS targets `supported()` now admits — native `aarch64-apple-darwin`, and
  a `--target x86_64-apple-darwin` cross-check using the same clang/Xcode SDK.
- **`cloudsync_from_source_windows`** (`windows-latest`, MSVC toolchain via
  `rust_install(platform: windows)` + `ilammy/msvc-dev-cmd`, mirroring the
  proven `windows_stt` job's setup): `cargo check -p cloudsync --features
  from-source` on `x86_64-pc-windows-msvc`.

Both are marked **PROVISIONAL / NON-BLOCKING** — not in the `ci` gate's
`needs:` list — for the same reason `windows_stt` is: this exact code path
has never run on a real macOS or Windows runner, so it cannot be proven green
from this Linux dev box, and a required check that's broken on day one is
worse than a provisional one. Promotion checklist: once each job has been
observed green across a few PRs, add it to `ci`'s `needs:` array.

YAML validated with `python3 -c "import yaml; yaml.safe_load(open(...))"` —
parses, both jobs present, `ci.needs` unchanged (neither new job added yet,
by design). It cannot be exercised further until the branch is pushed and a
workflow run actually happens on `macos-15`/`windows-latest`.

### 21.7 What's verified vs. not

**Verified on this (linux/x86_64) dev box:**
- `cargo check -p cloudsync` (default) — green, and a default `cargo check -p
  cloudsync -v` shows no `--cfg feature=...` for the crate (`features=[]`).
- `cargo check -p cloudsync --features from-source` — green; still links
  `cloudsync.so` and passes the existing `loads_bundled_cloudsync` test.
- `cargo test -p cloudsync --features from-source` — 6/6 (1 existing +
  5 new IPv6/agent_addr tests, after the round-2 audit fix in §21.9).
- `cargo test -p sync-p2p` — 25/25 across `agent`/`crypto`/`identity`/`peers`
  unit tests, `broker_protocol`, `iroh_transport` (unaffected by this lane —
  `crates/sync-p2p/src/agent.rs` was not touched).
- `cargo check -p desktop` (default) and `cargo check -p desktop --features
  sync` — both green.
- `supported()`'s logic for every target pair it now admits, indirectly via
  `cargo check -v`'s panic/no-panic behavior on this host's own
  `linux`/`x86_64` pair, and by code review of the `matches!` arms (an
  in-process `#[test]` for `supported()` itself was not added — it reads
  `CARGO_CFG_*` build-script env vars, which are only meaningfully set inside
  an actual build-script invocation for the target being built, so testing
  the other four pairs from a Rust unit test on this host would require
  faking those env vars rather than genuinely exercising the function against
  a real build).

**Updated after the first real CI run (run 33410001375, §21.10) — this
section originally said "not pushed yet"; it has now been pushed and run
once, and the result was exactly what Requirement 4 exists to catch: the
macOS reasoning above was wrong (missing `-framework Security`), caught by
the job, not by review.**

**NOT verified — genuinely still unknown:**
- **macOS**: unverified again after the §21.10 fix — the corrected link
  command (`-dynamiclib` + `-framework Security`) has not yet been run on a
  real macOS CI job. The first run proved the *previous* command wrong; it
  did not prove the new one right.
- **Windows**: completely unverified, unchanged since the original writeup —
  the first CI run never reached `cargo` at all (§21.10), so the `cl /LD` +
  `ws2_32.lib` link step remains exactly what it was before: documented
  `cl.exe`/`link.exe` behavior, not observed behavior. The job definition bug
  that caused this is fixed (§21.10), but that fix is *also* unverified until
  the next run.
- `linux/aarch64` — `supported()` admits it, but there is no aarch64
  toolchain on this dev box and no CI job for it either (unlike macOS/Windows,
  this repo has no existing aarch64-linux runner shape to reuse cheaply). Risk
  is lower than Windows/macOS (identical POSIX code path, same compiler
  family as the proven x86_64 build), but it is unverified, not "should be
  fine" — a claim this section already got burned on once for macOS.
- `linux/musl/x86_64` (§21.1) — pre-existing gap, not newly introduced.

**Do not read any of the above as "should work now."** Nothing in §21.2's
macOS fix or §21.10's Windows job fix has been observed green. The only
honest status for both platforms, as of this writing, is: fixed against a
real, specific, logged failure, and unverified again pending the next run.

### 21.8 Still open (unchanged by this lane)

- **⚠️ `plugins/db/Cargo.toml:52` still gates `sync-p2p`/`hypr-cloudsync` to
  `cfg(all(target_os = "linux", target_arch = "x86_64"))`.** This is the
  **next required step**, not a minor footnote: even once `crates/cloudsync`
  itself builds everywhere (and, as of §21.10, it does not yet — verified —
  anywhere but linux/x86_64), the desktop app's `sync` feature will not pull
  in `sync-p2p`/`hypr-cloudsync` on macOS, Windows, or linux/aarch64 at all,
  because this `[target.'cfg(...)'.dependencies]` table only lists
  `linux`+`x86_64`. Widening `crates/cloudsync/build.rs`'s `supported()`
  (§21.1) does nothing for the shipped app until this gate is widened to
  match — deliberately not done in this lane (SYNC-9 is scoped to "does the C
  code compile", not "is the app allowed to try it"), but it is the concrete,
  named next step, not a vague follow-up.
- **`strstr`-based JSON field lookup** (§12/§13.9, §21.4) — production
  hardening for a typed codec, deliberately not touched here.
- **No hostile-broker test fixture** (§12) — same reasoning.
- **Android** — needs `vendor/src/network/cacert.h`; out of scope for this
  lane, stays out of `supported()`.
- **Hub failover, blob-log GC, enabling more synced tables** — unrelated to
  this lane, not attempted.

### 21.9 Audit outcome (2026-08-31) — SYNC-9

`auditor` skill, `--coder claude-sonnet-5`, per-commit/per-file panels
(seats: `audit-minimax-m2.7`, `audit-gpt-oss:120b`; roster-selected, coder's
family excluded). Every finding was checked against the real code.

**Fixed (confirmed):**

- `agent_addr.h`'s bracket-stripping accepted `"[]:port"` (an empty
  bracketed host): `host_len` for `"[]"` is 2, which passes the `host_len >=
  2` guard, and stripping it yields an empty string with `ok=true` — a value
  `getaddrinfo()` may treat as "any address" rather than a failure. Caught by
  the gpt-oss seat; minimax's own walkthrough of the same input asserted
  `host_len == 1` for `"[]"` and read it as already excluded, which is
  arithmetically wrong (verified by hand: `"[]"` is 2 characters). Fixed by
  rejecting a `stripped_len` of 0; covered by
  `resolve_agent_addr_empty_brackets_fail_closed`.
- The `cloudsync_from_source_windows` CI job had no `timeout-minutes`, unlike
  the `windows_stt` job 12 lines above it (minimax). A hung MSVC link would
  otherwise run for up to the 6-hour GitHub Actions default. Added
  `timeout-minutes: 60`, then added the same to `cloudsync_from_source_macos`
  for consistency (minimax, second pass) — both are provisional jobs meant to
  be observed for a clean signal, not left to hang silently.
- The IPv6 test harness's scratch temp directories were never cleaned up
  (2-seat agreement: gpt-oss + minimax) — fixed, cleaned up on the normal
  completion path (a compile failure still panics before cleanup, leaving the
  harness source for debugging). gpt-oss separately flagged that the scratch
  dir name relied on the caller-supplied test `name` being unique, which
  cargo's same-process multi-threaded test execution does not enforce; added
  a monotonic counter so every call gets a unique directory regardless of
  `name`.

**Rejected as false positives (verified against the code):**

- **gpt-oss, `build.rs`:** "`.warnings(false)` hides genuine C compile
  issues." Pre-existing (not introduced by this diff — see §7 of this doc,
  "Warnings are off in build.rs [because] upstream code is not
  warning-clean"); not actioned.
- **minimax, `build.rs`:** "MSVC `/LD` link step is missing `kernel32.lib`."
  False — `cl.exe /LD` without `/NODEFAULTLIB` links the CRT's default
  library set (which pulls in `kernel32.lib`) automatically via
  `/DEFAULTLIB` directives embedded in the compiled objects; explicitly
  listing it is never required for a normal `cl.exe` invocation.
- **minimax, `network_p2p.c` base64 decoder:** claimed a heap overread for
  `b64_len % 4 == 1`. False — the arithmetic in the finding is wrong (`9 % 4
  = 1`, not `0`), and the decoder is only reached after an earlier check
  rejects any `b64_len` that is not a multiple of 4. Also pre-existing code
  from the §12 audit, untouched by this lane.
- **minimax, `agent_addr.h`:** "`host_len >= host_cap` should be `host_len >=
  host_cap - 1`, off-by-one buffer overflow." False — for `host_cap = N`, the
  check allows `host_len` up to `N - 1`; `memcpy` writes indices `0..N-2` and
  the NUL terminator lands at index `N - 1`, exactly filling an `N`-byte
  buffer. Standard, correct sizing, not an overflow.
- **gpt-oss, `agent_addr.h`:** "the `host_cap` bounds check runs before
  bracket-stripping, so a buffer sized to fit only the stripped host is
  rejected." True in the abstract, not actioned: the check is deliberately
  conservative (checked against the bracketed superset, not the eventual
  stripped length) per the explicit §12 instruction to keep this bounds
  check failing closed, and the only real caller (`network_p2p.c`) always
  passes a fixed 256-byte buffer — orders of magnitude larger than any real
  IPv6 literal (max 45 chars + 2 brackets), so the scenario is unreachable in
  practice.
- **gpt-oss, `agent_addr.h`:** "`strtol` accepts a leading-whitespace port
  string." True of `strtol` in isolation, not actioned: the port substring
  only ever originates from `NOTARE_SYNC_AGENT_ADDR`, whose only producer is
  Rust's `SocketAddr::to_string()` (`crates/sync-p2p/src/agent.rs:191`),
  which never emits whitespace — same "theoretically true, practically
  unreachable given the trusted single producer" shape as several §12
  findings already on record.
- **gpt-oss, `agent_addr.h`:** "unbracketed IPv6 literals with a trailing
  port are accepted as a valid host containing a colon." True of the
  last-colon-split design (pre-existing, not introduced here — bracketing is
  what makes IPv6:port unambiguous in the first place), and unreachable for
  the same reason: the only producer always brackets IPv6 via
  `SocketAddr::to_string()`.
- **gpt-oss, CI:** "60-minute timeout on `cloudsync_from_source_windows` is
  too short and will produce false-negative timeouts." Not actioned — the
  far heavier `windows_stt` job (Vulkan SDK, whisper.cpp, ONNX/DirectML)
  already runs reliably within the same 60-minute budget, and this job does
  only two `cargo check` invocations of one small crate.
- **minimax, `ipv6_bracket_host_split.rs`:** "the IPv4 test doesn't verify
  the returned port." False — it does (`if (port != 9999) { ... }`).

**Not usefully auditable:** the §21 documentation commit itself (this
section's own file, ~105k chars inlined — the doc is the single largest file
in the repo and a modified file's full text is always inlined). Both seats
returned 0 findings in under 15s each, well outside this tool's validated
size range (the largest payload that has ever produced a real finding in
this lane is under 37k chars) and far faster than either seat took on the
smaller `network_p2p.c` payload earlier in this same run. Per the auditor
skill's own documented failure mode, a fast 0-finding response on an
oversized payload reads as clean but is not distinguishable from a seat that
silently skipped deep reading — recorded here as an inconclusive audit, not
a clean one. `--only` cannot narrow a single-file audit further; no code
review substitute was applied beyond a manual re-read of the diff.

### 21.10 CI run 1 (2026-08-31) — the point of Requirement 4, proven

The branch was pushed and `desktop_ci.yaml` triggered via `workflow_dispatch`
(run `33410001375`). This is what Requirement 4 was for: both new jobs, and
this section's §21.2/§21.3/§21.7 "not locally verified" hedging, existed
specifically because reasoning about an unavailable platform is not the same
as observing it. The run confirmed that directly — the macOS reasoning was
wrong, and would have shipped wrong without this job.

**`cloudsync_from_source_macos` (job `99546898143`) — FAILED, a real bug,
now fixed (unverified again):**

```
error: failed to run custom build command for `cloudsync v0.1.0 (...)`
Undefined symbols for architecture arm64:
  "_SecRandomCopyBytes", referenced from: _cloudsync_uuid_v7 in ...utils.o
  "_kSecRandomDefault", referenced from: _cloudsync_uuid_v7 in ...utils.o
ld: symbol(s) not found for architecture arm64
clang: error: linker command failed with exit code 1
thread 'main' panicked at crates/cloudsync/build.rs:177:5:
failed to link cloudsync shared object from source
```

Root cause and fix recorded in §21.2. The bug was not in `network_p2p.c` (the
file this lane actually rewrote) — it was in the pre-existing vendored
`utils.c`'s `SecRandomCopyBytes` call, which needs `Security.framework` and
was never going to link with a bare `-dynamiclib`. §21.3's "macOS needed none
of this" claim was correct about `network_p2p.c` specifically and misleading
about the build as a whole; corrected there.

**`cloudsync_from_source_windows` (job `99546898295`) — FAILED, but not at
any code this lane wrote, and not a transient runner problem either.** The
job died inside `./.github/actions/rust_install`, specifically at that
action's own unconditional `cargo install trusted-signing-cli` step (for
`platform == 'windows'`, unrelated to what this job does) — **before**
`ilammy/msvc-dev-cmd` ran, so **before** `cargo` ever attempted the
`cloudsync` check. The Winsock2 port (§21.3) told this run nothing, one way
or the other.

The coordinator flagged that `windows_stt` failed identically in the same
run, and that three other jobs failed at `pnpm_install`, raising the
possibility of a run-wide infrastructure wobble. Checked directly against
both jobs' logs rather than assumed:

- `windows_stt`'s failure is **the identical error at the identical step**:
  `error occurred in cc-rs: failed to find tool "cl": program not found`,
  raised while `cargo install trusted-signing-cli` tries to compile
  `aws-lc-sys`'s `stdalign_check.c`.
- Both jobs set `CC: cl` / `CXX: cl` as **job-level env vars**, and both call
  `./.github/actions/rust_install` (which runs `cargo install
  trusted-signing-cli` unconditionally for `platform: windows`) **before**
  `ilammy/msvc-dev-cmd` (which is what puts `cl.exe` on `PATH` and sets
  `INCLUDE`/`LIB`). Setting `CC=cl` overrides `cc-rs`'s own MSVC-registry
  auto-detection (which does not need `cl.exe` on `PATH` — it locates MSVC
  directly via `vswhere`/the registry) and forces a bare-name `PATH` lookup
  for `cl` instead. At the point `rust_install`'s embedded step runs, `PATH`
  has no `cl.exe` yet in either job, so the lookup fails.

This is **not** the `pnpm_install`/transient-infra explanation — it is a
deterministic ordering bug, reproducible from the log alone, caused by a
specific env-var + step-order combination that this job copied from
`windows_stt` (which was described, incorrectly, as this job's "proven"
setup — it turned out not to have been proven at all, at least not in this
exact form, in this run). Fixed here, in this job only: dropped the `CC`/
`CXX: cl` override (`cc-rs`'s default auto-detection needs no override), and
moved `ilammy/msvc-dev-cmd` before `rust_install` as a second, independent
guarantee. **`windows_stt` was deliberately left untouched** — it is a
different lane's job, out of scope for this one, and the coordinator has not
asked for it to be fixed here; this section records the shared root cause so
whoever does fix it does not have to re-diagnose it.

**Status after this round: still zero observed-green platforms beyond
linux/x86_64.** The macOS fix and the Windows job-ordering fix are both
untested against a real runner as of this writing — say so explicitly rather
than letting the fixes read as "now it works." The next `desktop_ci` run
against this branch is the actual proof, not this section.

### 21.11 CI run 2 (2026-08-31) — macOS confirmed GO; a second, distinct Windows bug

Pushed and re-triggered (`gh workflow run desktop_ci.yaml --ref
feat/sync-9-crossplatform`, run `33411261329`).

**`cloudsync_from_source_macos` (job `99551111477`) — GREEN.** Both steps
passed: `cargo check -p cloudsync --features from-source` for
`aarch64-apple-darwin` (native) and `x86_64-apple-darwin` (cross), 3m41s
total. This is the first real observed proof that `crates/cloudsync`'s
`from-source` build compiles and links on macOS, on both admitted
architectures. §21.2/§21.7's "not locally verified" macOS hedging is
resolved: the `-dynamiclib` + `-framework Security` link command is now
**confirmed correct**, not just reasoned-through.

**`cloudsync_from_source_windows` (job `99551111594`) — FAILED again, but
past the first bug, at a second and different one.** `cl.exe` was found this
time (the §21.10 fix worked for that part — `getrandom`, `quote`,
`proc-macro2` all reached the compile stage while `cargo install
trusted-signing-cli` built), but the **link** step then failed:

```
error: linking with `link.exe` failed: exit code: 1
  = note: "C:\Program Files\Git\usr\bin\link.exe" "/NOLOGO" ...
  = note: /usr/bin/link: extra operand '...build_script_build...cgu.0.rcgu.o'
          Try '/usr/bin/link --help' for more information.
note: `link.exe` returned an unexpected error
note: the Visual Studio build tools may need to be repaired using the Visual Studio installer
error: could not compile `getrandom` (build script) due to 1 previous error
```

`rustc` resolved `link.exe` to **Git for Windows' own `/usr/bin/link.exe`**
— a POSIX hardlink utility, not a linker — instead of MSVC's, because this
job's `shell: bash` steps run through Git Bash, whose MSYS2 launcher
re-prepends its own `usr/bin` ahead of whatever `ilammy/msvc-dev-cmd` added
to `PATH`. This is a known interaction between `ilammy/msvc-dev-cmd` and
Git-Bash-shell steps on Windows GitHub Actions runners, not something
`GITHUB_PATH` ordering between steps can fix (Git Bash's own path
translation happens inside `bash.exe`'s own startup, on every invocation,
independent of accumulated `GITHUB_PATH` order).

Checked against this repo's own precedent to make sure the fix direction was
right rather than guessed: `release.yaml`'s `build-windows` job — the job
`windows_stt`'s header comment claims to model this whole setup on — does
**not** set `shell: bash` anywhere (its `CC`/`CXX: cl` are scoped to one
`pnpm -F desktop tauri build` step, after `msvc-dev-cmd`, running under the
default `pwsh`) and does **not** use `./.github/actions/rust_install` at all.
`windows_stt` (and this job, copied from it per the original task) diverged
from the actually-proven shape in exactly the two ways that caused both
rounds of failure: a job-level `shell: bash` default, and the shared
`rust_install` action's unrelated `trusted-signing-cli` install.

Fixed with the standard remedy for this exact interaction: a step that
deletes Git's own `link.exe` (`rm -f /usr/bin/link.exe`, harmless on an
ephemeral runner) before anything that needs MSVC's linker. Added between
`msvc-dev-cmd` and `rust_install`, so it protects both `rust_install`'s
embedded `trusted-signing-cli` build and this job's own `cargo check` step.
**`windows_stt` was not touched** (different lane's job, out of scope here);
it has not yet been observed reaching this second bug itself, since it still
fails at the first one (§21.10) every time — but it uses the identical
`shell: bash` + `msvc-dev-cmd` combination and would very likely hit the
identical `link.exe` shadowing once/if its `CC`/`CXX: cl` issue is fixed.
Recorded here so whoever fixes that job does not have to rediscover this.

**Status after this round: macOS is GO (observed, not reasoned). Windows
remains unverified** — the link.exe fix above has itself not yet been
observed against a real run. Do not read this section as "Windows now
works"; it is "Windows failed at a new, later point, with a specific fix
applied and not yet confirmed." The next `desktop_ci` run is still the
actual proof.
