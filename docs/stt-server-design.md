# Design: Notare Docker STT companion server (issue #14)

Date: 2026-07-17 · Status: Proposed · Milestone: v0.3

## 1. Summary

A self-hostable Docker server that runs Notare's speech-to-text on a
GPU-equipped machine (NVIDIA CUDA or AMD Vulkan) and serves transcription to
lightweight Notare desktop clients over the LAN, plus a web admin page for
downloading and managing models. The server is a **new binary crate in this
Cargo workspace** that reuses the existing engine, model-catalog, downloader,
integrity and HTTP-service crates almost verbatim — the same code the desktop
runs in-process, re-hosted on `0.0.0.0` with a fixed port and a management API
wrapped around it.

The single most important reuse fact: the desktop already builds its
`/v1/listen` endpoint from a **generic, engine-agnostic** service
(`hypr_transcribe_core::TranscribeService<E>`), and already contains an
`ExternalSTTActor` abstraction where a Notare client talks to an out-of-process
STT server over `http://host:port/v1`. The companion server is that external
server, generalized from `localhost` to a LAN host and given a model-admin API.

## 2. Goals / non-goals

### Goals (v1)

- One Docker image family that serves Deepgram-compatible `/v1/listen`
  (batch + WebSocket live) so an unmodified Notare client can point at
  `http://server:port` and transcribe.
- whisper.cpp as the **only** engine in v1, with GPU offload on CUDA and
  Vulkan (RDNA2 reference: AMD RX 6600 on CasaOS/Debian) and a CPU fallback
  image.
- A model-management API + minimal web admin page (list / download / delete /
  progress) reusing the existing catalog, downloader and CRC32/`.verified`
  integrity code.
- **Startup GPU-offload verification** that detects whisper.cpp's known silent
  CPU fallback on RDNA2 Vulkan and reports backend + timing in the admin UI and
  logs, refusing-or-warning per config.
- LAN-only security posture: optional bearer token, permissive CORS for the
  admin page, no TLS in v1 (documented).

### Non-goals (v1) — but the seams are designed in

- **Voxtral Mini** as a second engine (issue #16). Designed for: the
  `SttEngine`/`SttEngineSession` trait seam in `crates/transcribe-core/src/engine.rs`
  already hosts a second engine today (`crates/parakeet-onnx`), so Voxtral drops
  in as another `SttEngine` impl + a catalog entry with no server-架构 change.
- **pyannote speaker recognition** (issue #15). Designed for: keep diarization
  out of the `/v1/listen` hot path; add a separate `/v1/diarize` batch route and
  an optional model in the catalog. `crates/pyannote-local` /
  `crates/api-pyannote` already exist to borrow from.
- Cloud/off-LAN access, multi-tenant auth, TLS termination (put a reverse proxy
  in front if wanted), horizontal scaling.

## 3. Placement: a new workspace crate, not a separate repo

Add **`apps/stt-server`** (a binary crate) to the existing workspace
(`Cargo.toml` `members = ["apps/cli", "apps/desktop/src-tauri", "crates/*", ...]`;
this mirrors the existing `apps/cli` binary). Rationale for in-workspace over a
separate repo:

- **Zero-drift reuse via path deps.** Every crate the server needs is a
  workspace path dependency already wired in the root `Cargo.toml`
  `[workspace.dependencies]` table (`hypr-transcribe-core`,
  `hypr-transcribe-whisper-local`, `hypr-model-manager`,
  `hypr-model-downloader`, `hypr-local-model`, `hypr-whisper-local`,
  `owhisper-interface`, `owhisper-client`). A separate repo would have to pin git
  revisions of ~10 fast-moving crates and re-sync on every change.
- **The GPU feature flags already flow through the workspace.**
  `transcribe-whisper-local` exposes `vulkan`, `cuda`, `hipblas`, `openblas`,
  `metal`, `openmp` features that forward to `whisper-local` →
  `whisper-rs/{vulkan,cuda,...}` (see
  `crates/transcribe-whisper-local/Cargo.toml` and
  `crates/whisper-local/Cargo.toml`). The server crate re-exposes the same
  feature names; no build-system duplication.
- **Wire-protocol parity is guaranteed** because server and desktop share the
  exact `owhisper-interface` types (`ListenParams`, `StreamResponse`, batch/SSE
  messages). Protocol skew is impossible when both compile against one crate.
- **One toolchain, one `Cargo.lock`, one CI matrix.**

Cost and mitigation: the Docker build context is the whole workspace. Mitigate
with the existing `.dockerignore` (already excludes `target`, `node_modules`,
`dist`) plus `cargo-chef` for dependency-layer caching, and build only
`-p stt-server` (not the desktop/`tcc` graph, which is macOS-only).

Optional companion crate `crates/stt-server-core` if we want the router/model-
admin logic unit-testable without the binary; start with a single `apps/stt-server`
and split later only if tests demand it.

## 4. Reused building blocks (every claim cites an opened file)

| Concern | Reused code (real path) | Notes |
|---|---|---|
| `/v1/listen` batch + live WS + `/health` | `crates/transcribe-core/src/service/streaming.rs` — `TranscribeService::into_router(on_error)` mounts `GET /health` → `"ok"` and `route_service("/v1/listen")` (batch POST + WS upgrade). `LISTEN_PATH`/`HEALTH_PATH` consts. | Engine-generic; server uses it unchanged. |
| whisper.cpp engine binding | `crates/transcribe-whisper-local` — `pub type TranscribeService = hypr_transcribe_core::TranscribeService<LoadedWhisper>` and `engine.rs` (`LoadedWhisper`, `WhisperSession`, `arch()="whisper-local"`). | Drop-in `E` for the router. |
| In-process server scaffold to generalize | `crates/local-stt-server/src/lib.rs` (`LocalSttServer::start_whisper`), `axum_server.rs` (`LocalAxumServer`, graceful shutdown, CORS-any). | Today binds `Ipv4Addr::LOCALHOST:0`; server binds `0.0.0.0:<port>`. |
| Model load / unload / keep-alive | `crates/model-manager/src/manager.rs` (`ModelManager::get`, inactivity unload, `keep_alive`, `spawn_monitor`, warmup in `streaming.rs` builder). | One active model, lazy load, idle eviction — already what a server wants. |
| Download (single + multi-part) + progress | `crates/model-downloader` (`download_task/`, `download_task_progress.rs`, `downloads_registry.rs`, `archive.rs`, `manager.rs`; `DownloadableModel` trait in `model.rs`). | Multi-part + archive-unpack supported. |
| Integrity (size + CRC32 + `.verified` sidecar, quarantine) | `crates/model-downloader/src/integrity.rs` — `verify_model`, `ModelIntegrity {NotInstalled,Verified,PresentUnverified,Corrupt}`, per-file stamps. | Reused verbatim for `/api/models` state + startup reconciliation. |
| Model catalog (URLs, size, CRC32, languages) | `crates/whisper-local-model/src/lib.rs` (`WhisperModel`: 7 ggml models, `model_url`/`model_size_bytes`/`checksum`/`supported_languages`); aggregated in `crates/local-model/src/lib.rs` (`LocalModel`, `LocalModel::all()`). | Server ships a whisper-only view of the same catalog. |
| GPU backend enumeration (the offload-verification primitive) | `crates/whisper-local/src/ggml.rs` — `list_ggml_backends() -> Vec<GgmlBackend { kind: CPU/GPU/ACCEL, name, description, total_memory_mb, free_memory_mb }>`. **Only compiled in `--release` (`cfg(all(feature="actual", not(debug_assertions)))`)** — server images must be release builds. | Directly answers "which backend, how much VRAM". |
| GPU context params | `crates/whisper-local/src/model/actual.rs` — `WhisperContextParameters { gpu_device: 0, use_gpu: true, flash_attn: false }`. | whisper.cpp offloads the whole model when a GPU backend is compiled+present. |
| Wire protocol types | `crates/owhisper-interface` (`ListenParams`, `stream::StreamResponse`, `batch`, `batch_sse`, `openapi.rs`). | Shared with the desktop client. |
| Reference client (for tests) | `crates/owhisper-client` — `ListenClient::builder().api_base("http://host/v1").build_single()`, `from_realtime_audio(...)` (see `transcribe-whisper-local` test). | Server integration tests reuse it. |

## 5. Desktop client compatibility (the hard requirement)

**The "point the client at a URL" seam already exists end-to-end — including a
user-facing "Custom" STT provider.** A Notare client selects its STT endpoint
purely through a `baseUrl` (+ optional `apiKey`) that flows, unchanged, into
`owhisper_client::ListenClient::builder().api_base(...)` (live WS) and
`BatchClient::builder().api_base(...)` (batch). The relevant seam, top to bottom:

- **Provider config (persisted, user-editable).** `apps/desktop/src/stt/useSTTConnection.ts`
  returns `conn = { provider, model, baseUrl, apiKey }`. For local
  (`provider === "hyprnote"`) `baseUrl` is the in-process server's URL; for a
  **custom/cloud provider** `baseUrl = providerConfig.base_url`. There is already
  a `id: "custom"` provider entry (`apps/desktop/src/settings/ai/stt/shared.tsx`,
  ~L505) whose `base_url`+`api_key` the user types in, stored via
  `apps/desktop/src/settings/providers.ts` (`app_settings` row
  `ai_provider:stt:<id>`). **So the companion server plugs in as an STT provider
  config — no new client transport is needed.**
- **Same base for live and batch.** Live: `useStartListening.ts` →
  `ListenClient…api_base` (`plugins/dictation/src/session.rs`,
  `crates/listener-core/.../adapters.rs`). Batch: `useRunBatch.ts` →
  `BatchClient…api_base` (`crates/listener2-core/src/batch/simple.rs`). Both read
  the same `conn.baseUrl`.
- The in-process default path still works exactly as today: `local-stt`'s
  `internal.rs` binds `LOCALHOST:0`, reports `ServerInfo { url: "http://…/v1" }`,
  and `get_server_for_model` hands that URL back to the client. The `External`
  `ServerType`/`ExternalSTTActor` (`plugins/local-stt/src/server/external.rs`,
  `base_url = http://localhost:{port}/v1`) is the out-of-process precedent.

### ⚠ Wire-scheme gotcha — the one real client-side change a LAN server forces

`owhisper-client` decides ws/wss (and http/https) from the host, not from the
scheme you pass. `is_local_host` (`crates/owhisper-client/src/adapter/mod.rs`,
~L243–255) treats **only** `127.0.0.1`, `localhost`, `0.0.0.0`, `::1` as local;
`set_scheme_from_host`/`build_url_with_scheme` **upgrade any non-loopback host to
`wss`/`https`**. So a plaintext `http://homeserver:8080` live session is
rewritten to `wss://homeserver:8080/…` and fails against a non-TLS server. The
zod schema (`packages/store/src/zod.ts`, `aiProviderSchema`) also only requires
an `api_key` for `https:` URLs. **Implication:** to hit a plaintext LAN server,
we must either (a) run TLS on the companion server / front it with a reverse
proxy, or (b) relax the scheme heuristic to recognize LAN hosts (private RFC1918
ranges, `.local`/`.lan`, explicit `ws://`/`http://` opt-out). This is a small,
contained desktop-side change and is the deciding factor for open question 2.

The compatibility contract the companion server must satisfy is exactly the
surface `transcribe-core` already serves:

- `POST  {base}/v1/listen?…` — batch. Body = raw audio (e.g. `audio/wav`);
  `Accept: text/event-stream` switches to SSE progress
  (`crates/transcribe-core/src/transport.rs::batch_sse_response`). Query =
  `ListenParams` (`language`, `keywords`, `channels`, `sample_rate`,
  `custom_query[redemption_time_ms]`, …), parsed by `parse_listen_params`.
- `GET   {base}/v1/listen?channels=&sample_rate=` **(WebSocket upgrade)** —
  live streaming; client sends `MixedMessage::Audio` frames, server replies
  `StreamResponse` JSON (see `transport.rs::send_ws`).
- `GET   {base}/health` → `"ok"`.

**Client-side change to make (small, desktop-side, named here — the server crate
itself needs none):** configure the companion server as a **Custom STT provider**
(`base_url = http(s)://<lan-host>:<port>/v1`, optional `api_key`) via the
existing provider settings. The only code change is the wire-scheme relaxation in
`crates/owhisper-client/src/adapter/mod.rs` (or shipping TLS) described in the
gotcha above. No new client transport, no new `ServerType`.

## 6. Server architecture

Single Tokio + axum binary (axum is the in-repo HTTP stack — `transcribe-core`,
`local-stt-server` and the `otel` services all use axum; no new framework).

```
apps/stt-server/
  main.rs        -- clap args/env, tracing init, bind 0.0.0.0:<port>, build Router, serve
  config.rs      -- Config from env/flags (port, model dir, token, require_gpu, cors)
  router.rs      -- merge transcribe-core router + /api/* + static admin + middleware
  admin/         -- /api/models, /api/status handlers (thin wrappers over reused crates)
  probe.rs       -- GPU offload verification (list_ggml_backends + timed probe)
  assets/        -- embedded static admin SPA (index.html + app.js + app.css)
```

The core router is literally:

```rust
let core = hypr_transcribe_whisper_local::TranscribeService::builder()
    .model_path(active_model_path)          // from ModelManager / catalog
    .build()
    .into_router(|err| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

let app = core                              // /health + /v1/listen (batch + WS)
    .merge(admin_router(state))             // /api/*  + static admin
    .layer(cors_layer())                    // reuse local-stt-server::cors_layer style
    .layer(auth_middleware(state.token));   // optional bearer, /api + optionally /v1
```

### Endpoints

| Method | Path | Purpose | Reuse |
|---|---|---|---|
| `GET` | `/health` | Liveness → `"ok"` | `transcribe-core` HEALTH_PATH |
| `POST` | `/v1/listen` | Batch transcription (+SSE via `Accept`) | `transcribe-core` batch |
| `GET` (WS) | `/v1/listen` | Live streaming transcription | `transcribe-core` streaming |
| `GET` | `/api/status` | Backend in use, GPU-offload verification result, VRAM total/free, loaded model, uptime, engine `arch` | `list_ggml_backends`, `probe.rs`, `ModelManager` |
| `GET` | `/api/models` | Catalog + per-model `ModelIntegrity` (NotInstalled/Verified/PresentUnverified/Corrupt) | `local-model` catalog + `integrity::verify_model` |
| `POST` | `/api/models/{id}/download` | Start (or resume) download; returns 202 | `model-downloader` download task |
| `GET` (WS/SSE) | `/api/models/{id}/progress` | Download progress stream | `download_task_progress` + `downloads_registry` |
| `POST` | `/api/models/{id}/cancel` | Cancel in-flight download | `model-downloader` manager |
| `DELETE` | `/api/models/{id}` | Delete on disk | `DownloadableModel::delete_downloaded` |
| `POST` | `/api/models/{id}/activate` | Set the loaded/default model | `ModelManager::register`+`set_default` (rebuild service) |
| `GET` | `/` (+ assets) | Static admin SPA | embedded assets |

`/api/status` example payload:

```json
{
  "engine": "whisper-local",
  "loaded_model": "QuantizedSmall",
  "backend": { "kind": "GPU", "name": "Vulkan0",
               "description": "AMD Radeon RX 6600 (RADV NAVI23)",
               "total_memory_mb": 8176, "free_memory_mb": 7100 },
  "gpu_offload": { "verified": true, "method": "probe",
                   "probe_realtime_factor": 12.4, "note": null },
  "uptime_secs": 3600
}
```

## 7. GPU story

### Build matrix (three Dockerfiles, one crate)

| Image | Base | Cargo features | Runtime |
|---|---|---|---|
| `Dockerfile.cpu` | `debian:bookworm-slim` | none (`openmp` optional) | portable; the shakedown image on **this WSL box** |
| `Dockerfile.vulkan` | Vulkan/Mesa runtime (RADV) on Debian | `--features vulkan` | `--device /dev/dri --group-add video`; **coruscant RX 6600 target** |
| `Dockerfile.cuda` | `nvidia/cuda:*-runtime` | `--features cuda` | `nvidia-container-toolkit` / `--gpus all` |

- Features map straight through: `stt-server` re-exposes `vulkan`/`cuda`/`hipblas`
  → `transcribe-whisper-local/{vulkan,cuda,hipblas}` →
  `whisper-local/{vulkan,cuda,hipblas}` → `whisper-rs/*` (verified in the three
  `Cargo.toml`s). The in-repo feature is named **`vulkan`** (not `gpu-vulkan`).
- **Release builds are mandatory** for all GPU images: `list_ggml_backends()` is
  `cfg`-gated to `not(debug_assertions)` (`crates/whisper-local/src/ggml.rs`), so
  a debug image cannot report its backend.
- Multi-stage builder with `cargo-chef`; final stage copies only the
  `stt-server` binary + embedded assets + the model volume mount.

### RDNA2 silent-CPU-fallback verification (the required design)

Two independent checks, both surfaced in `/api/status` and logs:

1. **Backend presence (cheap, at startup).** Call `list_ggml_backends()`. If the
   image was built with a GPU feature but the returned set contains **no device
   with `kind == "GPU"` (or `ACCEL`)**, or the GPU device reports
   `total_memory_mb == 0`, that is a hard signal of missing ICD / no
   `/dev/dri` / driver mismatch.
2. **Timed offload probe (authoritative).** Load the active model and transcribe
   a fixed, bundled short audio clip once at boot; measure wall-clock and compute
   a **realtime factor** (audio_seconds / processing_seconds). whisper.cpp on a
   real GPU is many× realtime; a silent CPU fallback on RDNA2 collapses toward
   ~1× or below. If the factor is under a per-backend threshold **while a GPU
   backend was expected**, flag `gpu_offload.verified = false` with
   `note = "possible silent CPU fallback"`.
   - Also **do not suppress the ggml log** in the server (the desktop installs a
     noop log callback in `whisper-local/src/model/actual.rs::suppress_log`);
     the server captures ggml's init lines (which name the chosen backend) into
     tracing as a third corroborating signal.

**Policy** (config `require_gpu`, see open question 5): when a GPU image detects
fallback, either **refuse to start** (fail-fast for a headshot GPU box) or
**warn-and-serve on CPU** (degraded but working). Default proposed:
warn-and-serve, with a red banner in the admin UI, because a working CPU
transcription beats a dead server; `require_gpu=true` flips it to refuse.

## 8. Model management, concurrency, storage

### Storage layout (Docker volume)

Mount one volume at `/data`. Layout mirrors the desktop's `…/models` base that
`ModelDownloadManager`'s `models_base()` already expects
(`plugins/local-stt/src/ext.rs::TauriModelRuntime::models_base`):

```
/data/
  models/stt/ggml-small-q8_0.bin
  models/stt/ggml-small-q8_0.bin.verified     # CRC32 sidecar (integrity.rs)
  models/stt/ggml-large-v3-turbo-q8_0.bin
  config.json                                  # active model, token, settings
```

`.part-<generation>` temp files during download come from
`model-downloader/src/download_paths.rs`; multi-part models install as a
directory (already handled by `integrity::verify_parts`).

### Startup reconciliation

On boot, run `integrity::verify_model` over every installed catalog model
(same discipline as ADR-0002 §3). Quarantine `Corrupt` files, log, and expose
state via `/api/models` so the admin UI can prompt a re-download.

### Concurrency model

- **One active model** at a time (`ModelManager` holds a single `ActiveModel`);
  a request naming a different model triggers a serialized reload. This matches
  a single-GPU box.
- **A bounded transcription semaphore** (default = 1 for GPU, small N for CPU)
  in front of the whisper worker: whisper.cpp uses one GPU context, so parallel
  batch jobs must queue, not thrash VRAM. Batch already runs on
  `spawn_blocking` (`transcribe-core/src/service/batch.rs`); the semaphore wraps
  that. Live WS sessions are cheap to hold but share the same compute — cap
  concurrent live sessions via `ConnectionManager` (already in
  `TranscribeService`).
- **Idle unload**: `ModelManager`'s `inactivity_timeout` + `spawn_monitor`
  already free VRAM after inactivity; `keep_alive` is pinged per request.
- **Timeouts**: axum/`tower-http` request timeout on `/v1/listen` batch; WS idle
  timeout via `ConnectionManager`. Model-load timeout surfaces the existing
  `model_load_failed` JSON error (`transcribe-core` batch path returns it).

## 9. Web admin page

**Decision: a single embedded static page, no separate frontend build.** The
repo's frontend is a large pnpm/turbo/React monorepo (`apps/desktop`,
`apps/web`, `packages/*`); adding `stt-server` to that graph would force a Node
build stage into every Docker image. The admin surface is three read-mostly
screens (Status, Models, Logs) driven entirely by the `/api/*` JSON. So:

- Ship a hand-written `index.html` + vanilla `app.js` + `app.css`, **embedded in
  the binary** (via `rust-embed`/`include_str!`) and served by the same axum
  router at `/`. No Node in the image, self-contained binary.
- The page polls `/api/status`, lists `/api/models` with per-model integrity
  badges and Download/Delete buttons, subscribes to `/api/models/{id}/progress`,
  and prominently renders the GPU-offload verification result (green
  "GPU: Vulkan0 (RX 6600), 12.4× realtime" / red "CPU fallback detected").
- Escalation path if the UI grows: add a tiny Vite app under `apps/stt-server/ui`
  built to static assets in a separate CI step — but not for v1.

## 10. Security

- **LAN-only posture.** Bind `0.0.0.0:<port>` but document that the box must sit
  behind the home firewall / not be port-forwarded. `SECURITY.md` conventions
  apply.
- **Optional bearer token.** `NOTARE_STT_TOKEN` (or `config.json`) enables an
  axum middleware requiring `Authorization: Bearer <token>` on `/api/*` and
  (optionally) `/v1/*`. The desktop's `Connection`/`ExternalSTTArgs` already
  carry an `api_key`, so the client side is ready. Token is shown in the admin UI
  on first boot.
- **No TLS in v1** (documented) — **but note the client wire-scheme gotcha
  (§5):** `owhisper-client` upgrades non-loopback hosts to `wss`/`https`. Plaintext
  v1 therefore requires the small desktop-side scheme relaxation (recognize LAN
  hosts / honor explicit `http://`); the TLS alternative is to front with
  Caddy/nginx (the repo already has a `otel/caddy/Dockerfile` to copy). This is a
  v1 decision, not a v2 nicety (open question 2).
- **CORS**: reuse the permissive `cors_layer()` shape from
  `local-stt-server/src/lib.rs` for `/v1` (desktop clients from any origin), and
  same-origin for the admin page; expose an allowed-origins config for tightening.
- Reaffirm Notare's **no-telemetry** principle (ADR-0002): the server links no
  analytics; the only outbound traffic is model downloads from the catalog URLs.

## 11. Phased implementation plan

Each phase is independently shippable and testable on **this WSL box (CPU image)**
and on **coruscant (RX 6600 / Vulkan image)**. Sizes are rough (S≈½ day,
M≈1–2 days, L≈3+ days of agent time).

- **Phase 1 — Serving core (M). FOUNDATION, must land first.**
  `apps/stt-server` crate: clap/env config, tracing, bind `0.0.0.0:<port>`,
  build the `transcribe-whisper-local` router (`/health` + `/v1/listen`) against
  a model path from `/data`, `Dockerfile.cpu`. **Test:** `owhisper-client`
  batch + live against the container; existing `transcribe-whisper-local`
  integration test as the template. Freezes the `/api/*` contract for parallel
  work.

- **Phase 2 — Model management API (M).** `/api/models` (list + integrity),
  download/cancel/delete, `/api/models/{id}/progress`, `/api/models/{id}/activate`
  (+ live model swap), startup reconciliation. Pure wrappers over
  `model-downloader` + `integrity` + `ModelManager`. **Test on CPU box.**
  *Parallelizable with Phase 3 and 4 once Phase 1 fixes the contract.*

- **Phase 3 — Web admin SPA (S–M).** Embedded static Status/Models/Logs page
  driven by `/api/*`. **Test on CPU box.** *Parallel with Phase 2 (contract-first)
  and Phase 4.*

- **Phase 4 — GPU images + offload verification (L).** `Dockerfile.vulkan` and
  `Dockerfile.cuda` (release, feature-gated), `probe.rs` (backend presence +
  timed probe + ggml-log capture), `require_gpu` policy, `/api/status` backend
  reporting. **Test on coruscant RX 6600** (the RDNA2 fallback scenario) and any
  CUDA box. *Depends on Phase 1 binary; otherwise independent of 2/3.*

- **Phase 5 — Security + desktop client seam + docs (M).** Bearer-token
  middleware + CORS config; desktop-side: register the server as a **Custom STT
  provider** and resolve the wire-scheme gotcha (§5) — either relax
  `is_local_host`/scheme-upgrade in `crates/owhisper-client/src/adapter/mod.rs`
  for LAN hosts, or ship/require TLS; operator README (volumes,
  `--device /dev/dri`, `--gpus all`, firewall, token). *The desktop seam is
  independent of server internals and can start as early as Phase 2.*

**Parallelization summary:** Phase 1 is the gate. After it, three coding agents
can run concurrently — Agent A: Phase 2 (model API), Agent B: Phase 3 (admin UI,
contract-first), Agent C: Phase 4 (GPU/Docker). Phase 5's client-seam half can
run on a fourth track against the desktop from the moment the `/v1/listen`
contract is frozen.

## 12. Open questions for Abhishek (decisions needed)

1. **Discovery & default port.** Fixed default port (proposed `8383`) with
   manual `http://host:port` entry in Notare settings — or do we also want mDNS/
   zeroconf auto-discovery so the desktop finds the server on the LAN?
2. **Transport + auth.** The client upgrades non-loopback hosts to `wss`/`https`
   (§5 gotcha). Do we ship **plaintext HTTP + a small `owhisper-client` LAN-scheme
   relaxation** (simplest for a home LAN), or **require TLS** (front with Caddy)?
   And relatedly: bearer token **required by default** (generated first boot,
   shown in admin) or **off by default**? Recommended: plaintext + LAN-scheme
   relaxation + token-off-by-default for v1.
3. **Model source.** Keep pulling from the existing hyprnote S3 mirror + Hugging
   Face (`large-v3-turbo` is HF-only, 403 on S3 — see
   `whisper-local-model/src/lib.rs`), or self-host a model mirror on coruscant?
4. **Admin UI approach.** Confirm the embedded vanilla static page (no pnpm/turbo,
   no Node in the image) over adding a Vite app to the workspace. Recommended:
   embedded.
5. **GPU-fallback policy default.** On a GPU image that detects silent CPU
   fallback (RDNA2), **refuse to start** (fail-fast) or **warn-and-serve on CPU**
   (degraded)? Recommended: warn-and-serve, `require_gpu=true` to fail-fast.
