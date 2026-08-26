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