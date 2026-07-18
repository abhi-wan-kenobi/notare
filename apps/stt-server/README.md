# notare-stt-server (Phase 1 + Phase 2 + Phase 3)

A standalone LAN transcription server: it hosts the same generic,
engine-agnostic `hypr_transcribe_core::TranscribeService` router the Notare
desktop app runs in-process for its local STT, bound to a configurable
`host:port` instead of `LOCALHOST:0`. Design: `docs/stt-server-design.md`.

Phase 1 shipped `/health` + `/v1/listen` (batch + WebSocket) with the
whisper.cpp CPU engine, plus read-only `/api/status` and `/api/models`.
**Phase 2 adds real model management**: download, delete, activate, download
progress, and startup integrity reconciliation — all thin wrappers over the
existing `hypr-model-downloader` (CRC32 + `.verified` sidecar integrity) and
`hypr-model-manager`/`TranscribeService` (lazy load, background warmup)
crates; see `docs/stt-server-design.md`'s Phase 2 addendum for exactly how.
**Phase 3 adds `GET /`** — a single self-contained, embedded web admin page
(no Node/build step, vanilla HTML+CSS+JS via `include_str!`, see
`src/assets/index.html`). It polls `/api/status` + `/api/models` and drives
the real Phase 2 mutation routes directly (install/cancel/delete/activate,
live progress bar) — there is no "Coming soon" placeholder state in this
merged tree, since Phase 2 landed alongside it. GPU images are still a later
phase (design doc §11).

## Run

```sh
cargo run -p stt-server -- --model-dir ./data/models --model QuantizedSmall
```

Or via env vars only (what the Dockerfile uses):

```sh
NOTARE_STT_HOST=0.0.0.0 \
NOTARE_STT_PORT=8383 \
NOTARE_STT_MODEL_DIR=/data/models \
NOTARE_STT_MODEL=QuantizedSmall \
NOTARE_STT_REQUIRE_GPU=false \
cargo run -p stt-server
```

Add `NOTARE_STT_TOKEN=<some-secret>` to any of the above to turn on the
optional bearer-token gate (see "Security" below) — omit it (the default)
to leave the server unauthenticated on the LAN.

CLI flags take precedence over env vars, which take precedence over the
defaults below.

## Config (flags / env vars)

| Flag | Env var | Default | Notes |
|---|---|---|---|
| `--host` | `NOTARE_STT_HOST` | `0.0.0.0` | Use `127.0.0.1` to restrict to loopback. |
| `--port` | `NOTARE_STT_PORT` | `8383` | Adopted default port (design doc §12 Q1). |
| `--model-dir` | `NOTARE_STT_MODEL_DIR` | `./data/models` | Base dir; a model installs at `<model-dir>/stt/<file>`, matching the desktop's `models_base` layout. |
| `--model` | `NOTARE_STT_MODEL` | `QuantizedSmall` | One of the `WhisperModel` catalog ids: `QuantizedTiny`, `QuantizedTinyEn`, `QuantizedBase`, `QuantizedBaseEn`, `QuantizedSmall`, `QuantizedSmallEn`, `QuantizedLargeTurbo`. |
| `--require-gpu` | `NOTARE_STT_REQUIRE_GPU` | `false` | Flag; reserved for Phase 4's GPU offload-verification refusal policy. No-op today — this image only ever serves on CPU. |
| `--token` | `NOTARE_STT_TOKEN` | unset | Optional shared-secret gate on `/v1/listen` + the four `/api/models/*` mutation routes (`download`/`cancel`/`activate`/`DELETE`). Off by default — see "Security" below. |

## Security — read this before exposing the port to anything but your own LAN

> ⚠ **This server is plaintext HTTP with no authentication by default, and
> binds `0.0.0.0` (every network interface) out of the box.** That is a
> deliberate v1 tradeoff, not an oversight — see the design doc §10/§12 Q2 —
> but it means **you** are the one keeping it isolated, not the server:
>
> - **Never port-forward this to the internet.** Keep it on a trusted home/
>   office LAN or a tailnet (Tailscale etc.) only.
> - If you need it reachable from outside your LAN, put it behind Tailscale
>   or another VPN/mesh network — do not open the port on your router.
> - `--host 127.0.0.1` restricts it to loopback if you never need another
>   device on the LAN to reach it.

An independent security review (`SECURITY-REVIEW.md`) found and this release
fixed two pre-release issues that undermined even that LAN-only isolation:

- **CORS used to be wide open (`cors::Any`)** and the `/v1/listen` WebSocket
  upgrade didn't check the `Origin` header at all — meaning *any website a
  user had open in a browser tab* could activate/delete models or open a
  live-transcription session against this server, from JavaScript, with no
  interaction. Both are now gated by one shared allowlist (Tauri's desktop
  webview origins + localhost dev origins; see
  `apps/stt-server/src/router.rs::cors_layer` and
  `crates/transcribe-core/src/origin.rs`) — not arbitrary origins.
- **`/api/status` used to return the model's absolute filesystem path**
  (leaking the host's home-dir path, OS username, and OS type to anything
  that could reach it, which the CORS bug above made "any webpage"). Fixed
  — the response only carries `id`/`file` now.

**Optional extra hardening: `NOTARE_STT_TOKEN`.** Set this env var (or
`--token`) to require `Authorization: Bearer <token>` on `/v1/listen` and
the mutation routes. It's off by default and does not change the LAN-only
posture above — think of it as a second gate for a shared-LAN scenario (a
flat, an office), not a substitute for network isolation. No desktop-side
change is needed: the "Custom" STT provider's optional `api_key` field
already sends `Authorization: Bearer <api_key>` on every request — set the
same value in both places.

**Known, deferred (not fixed in this pass — see `SECURITY-REVIEW.md`):** a
same-LAN client can still force-disconnect the one active `/v1/listen`
session by opening a new one (SEC-03, DoS via the intentional single-session
design — origin/token gates above cut off the *browser* vector for this,
not a same-LAN native client); nothing caps how many of the 7 catalog
models can download concurrently (SEC-06, low severity, small catalog).
Both are tracked for a later pass, not release blockers.

## Installing a model

Two ways:

1. **`POST /api/models/{id}/download`** (Phase 2, recommended — see below).
2. Manually, placing a whisper.cpp ggml file yourself at the catalog path:

   ```sh
   mkdir -p ./data/models/stt
   curl -L -o ./data/models/stt/ggml-small-q8_0.bin \
     https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-small-q8_0.bin
   ```

(File names + URLs + CRC32 per model: `crates/whisper-local-model/src/lib.rs`.)
Without a model installed the server still starts and answers `/health` and
`/api/status`; `/v1/listen` returns a `model_load_failed` JSON error until a
model is present at the configured path. On boot, every installed catalog
model is re-verified (existence + size + CRC32) and quarantined to
`*.corrupt` if it fails — see "Startup reconciliation" below.

## Endpoints

- `GET /` — embedded web admin page (Phase 3): server identity/version/
  uptime, engine + GPU backend list + offload state, loaded model, and the
  models table (size, languages if the catalog exposes them, integrity,
  install/delete/activate actions + download progress bar).
- `GET /health` — liveness, always `"ok"` (no model required).
- `POST /v1/listen?channels=&sample_rate=` — batch transcription. `Accept:
  text/event-stream` switches to SSE progress. Same contract as the
  desktop's in-process server. Dispatches to whichever model was last
  `activate`d (§ below) — no restart needed after an activation.
- `GET /v1/listen?channels=&sample_rate=` (WebSocket upgrade) — live
  streaming transcription.
- `GET /api/status` — version, engine, `loadedModel` (the currently active
  model, or `null` if it isn't installed/verified — see `activate`), on-disk
  model integrity, GPU backend list (empty on CPU image / debug
  builds), `requireGpu` config flag, `gpuOffload` status (`"verified" | "cpu" | "unknown"`),
  `probeRealtimeFactor` (timed verification probe result in realtime factor ratio, or `null` if none run), uptime.
- `GET /api/models` — the whisper.cpp catalog with per-model on-disk
  integrity (`notInstalled` / `verified` / `presentUnverified` / `corrupt`)
  and a `progress` snapshot (see `.../progress` below).
- `POST /api/models/{id}/download` — start an async download.
  `404` unknown id · `409 already_downloading` if one is already in flight ·
  `200 {"status":"alreadyInstalled"}` no-op if it's already installed ·
  `202 {"status":"downloading"}` once a new download has started.
- `GET /api/models/{id}/progress` — poll download/install status for one
  model: `{"id", "progress": {"status": "idle"|"downloading"|"completed"|
  "failed"|"corrupt", "percent"?, "detail"?}}`. `404` unknown id. This is a
  **plain polled JSON endpoint, not the WS/SSE stream** originally sketched
  in the design doc — see the Phase 2 addendum there for why polling was
  chosen (same pattern `/api/status` already uses).
- `POST /api/models/{id}/cancel` — cancel an in-flight download. `404`
  unknown id · `409 not_downloading` if nothing is in flight ·
  `200 {"status":"cancelled"}`.
- `DELETE /api/models/{id}` — remove a model's files + `.verified` sidecar.
  `404` unknown id · `409 model_in_use` if it's the currently active/loaded
  model (activate a different one first) ·
  `200 {"status":"notInstalled"}` no-op if it wasn't installed ·
  `200 {"status":"deleted"}` on success.
- `POST /api/models/{id}/activate` — make this the model `/v1/listen` serves
  and `/api/status.loadedModel` reports. `404` unknown id ·
  `409 model_not_installed` / `409 model_corrupt` if it fails integrity
  verification (download it first) · `200 {"status":"activated","integrity":
  ...}` on success.

All error responses share `/v1/listen`'s envelope:
`{"error": "<code>", "detail": "<message>"}`
(`hypr_transcribe_core::json_error_response`).

### Startup reconciliation

On boot, before the listener binds, every installed catalog model is
re-verified against disk (existence + size + CRC32, same discipline as the
desktop's ADR-0002) via `hypr_model_downloader::ModelDownloadManager::
reconcile`. Anything that fails is quarantined by renaming it to
`<file>.corrupt` (sidecar `.verified` stamp removed) so a subsequent
`GET /api/models` correctly reports it as `notInstalled` and a re-download
via `POST /api/models/{id}/download` starts clean.

## curl examples

```sh
curl http://127.0.0.1:8383/health
# ok

curl http://127.0.0.1:8383/api/status | jq
curl http://127.0.0.1:8383/api/models | jq '.models[] | {id, active, integrity, progress}'

# Download the smallest model, poll progress, activate it.
curl -X POST http://127.0.0.1:8383/api/models/QuantizedTiny/download | jq
watch -n1 'curl -s http://127.0.0.1:8383/api/models/QuantizedTiny/progress | jq'
curl -X POST http://127.0.0.1:8383/api/models/QuantizedTiny/activate | jq
curl http://127.0.0.1:8383/api/status | jq '.loadedModel'

# While a download is still in flight, POST .../cancel instead of waiting.
curl -X POST http://127.0.0.1:8383/api/models/QuantizedTiny/cancel | jq

# activate a different model first if you want to delete the active one.
curl -X DELETE http://127.0.0.1:8383/api/models/QuantizedTiny | jq

curl -X POST "http://127.0.0.1:8383/v1/listen?channels=1&sample_rate=16000" \
  -H "content-type: audio/wav" --data-binary @audio.wav | jq
```

## Docker

The companion server has three Docker image options depending on your hardware:

### 1. CPU (no GPU offload)
```sh
docker build -f apps/stt-server/Dockerfile.cpu -t notare-stt-server:cpu .
docker run --rm -p 8383:8383 \
  -v notare-stt-models:/data/models \
  notare-stt-server:cpu
```

### 2. AMD Vulkan GPU Offloading
Enables whisper.cpp's `vulkan` feature. Requires mounting `/dev/dri` and adding the container user to the host's `video`/`render` group.
```sh
docker build -f apps/stt-server/Dockerfile.vulkan -t notare-stt-server:vulkan .
docker run --rm -p 8383:8383 \
  --device /dev/dri \
  --group-add video \
  -v notare-stt-models:/data/models \
  -e NOTARE_STT_MODEL_DIR=/data/models \
  notare-stt-server:vulkan
```
*Note for RDNA2 (e.g. RX 6600):* If silent CPU fallback is detected, the admin panel surfaces a `CPU Fallback` state. Run with `NOTARE_STT_REQUIRE_GPU=true` to fail startup if Vulkan is not working.

### 3. NVIDIA CUDA GPU Offloading
Enables whisper.cpp's `cuda` feature. Requires the NVIDIA Container Toolkit.
```sh
docker build -f apps/stt-server/Dockerfile.cuda -t notare-stt-server:cuda .
docker run --rm -p 8383:8383 \
  --gpus all \
  -v notare-stt-models:/data/models \
  -e NOTARE_STT_MODEL_DIR=/data/models \
  notare-stt-server:cuda
```


## Tests

```sh
cargo check -p stt-server
cargo test -p stt-server
```

Do **not** run `cargo check --workspace` or `--all-features` for this crate
— both are known-broken on Linux dev boxes in this repo for reasons
unrelated to `stt-server` (`--all-features` pulls a BLAS feature nothing
here uses; `--workspace` reaches `crates/tcc`, which is macOS-only Swift
with no Linux cfg gate). `cargo check -p stt-server` / `cargo test -p
stt-server` are the right scoped commands.
