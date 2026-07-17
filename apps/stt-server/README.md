# notare-stt-server (Phase 1)

A standalone LAN transcription server: it hosts the same generic,
engine-agnostic `hypr_transcribe_core::TranscribeService` router the Notare
desktop app runs in-process for its local STT, bound to a configurable
`host:port` instead of `LOCALHOST:0`. Design: `docs/stt-server-design.md`.

Phase 1 scope only: serve `/health` + `/v1/listen` (batch + WebSocket) with
the whisper.cpp CPU engine, plus a read-only `/api/status` and `/api/models`.
Model download/delete/activate, the web admin page, and GPU images are later
phases (see the design doc §11) — their `/api/*` routes exist now (frozen
contract) but answer `501 Not Implemented`.

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

There is **no auth token and no TLS in Phase 1** (adopted decision — see the
design doc §12 Q2). Treat the server as LAN-only; do not port-forward it.

## Installing a model (Phase 1: manual)

Model management (`/api/models/{id}/download` etc.) is Phase 2. For now,
place a whisper.cpp ggml file yourself at the catalog path:

```sh
mkdir -p ./data/models/stt
curl -L -o ./data/models/stt/ggml-small-q8_0.bin \
  https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-small-q8_0.bin
```

(File names + URLs + CRC32 per model: `crates/whisper-local-model/src/lib.rs`.)
Without a model installed the server still starts and answers `/health` and
`/api/status`; `/v1/listen` returns a `model_load_failed` JSON error until a
model is present at the configured path.

## Endpoints

- `GET /health` — liveness, always `"ok"` (no model required).
- `POST /v1/listen?channels=&sample_rate=` — batch transcription. `Accept:
  text/event-stream` switches to SSE progress. Same contract as the
  desktop's in-process server.
- `GET /v1/listen?channels=&sample_rate=` (WebSocket upgrade) — live
  streaming transcription.
- `GET /api/status` — version, engine, loaded model (or `null`), on-disk
  model integrity, GPU backend list (empty on this CPU image / debug
  builds — `list_ggml_backends()` is release-build-only), uptime.
- `GET /api/models` — the whisper.cpp catalog with per-model on-disk
  integrity (`notInstalled` / `verified` / `presentUnverified` / `corrupt`).
- `POST /api/models/{id}/download`, `GET /api/models/{id}/progress`,
  `POST /api/models/{id}/cancel`, `DELETE /api/models/{id}`,
  `POST /api/models/{id}/activate` — routes exist (contract frozen for
  Phase 2/3/4 parallel work) but currently return `501` with the shared
  error envelope: `{"error": "not_implemented", "detail": "..."}`. This is
  the same shape `/v1/listen` already uses for its errors
  (`hypr_transcribe_core::json_error_response`).

## curl examples

```sh
curl http://127.0.0.1:8383/health
# ok

curl http://127.0.0.1:8383/api/status | jq
curl http://127.0.0.1:8383/api/models | jq '.models[] | {id, active, integrity}'

curl -X POST "http://127.0.0.1:8383/v1/listen?channels=1&sample_rate=16000" \
  -H "content-type: audio/wav" --data-binary @audio.wav | jq
```

## Docker (CPU)

```sh
docker build -f apps/stt-server/Dockerfile.cpu -t notare-stt-server:cpu .
docker run --rm -p 8383:8383 \
  -v notare-stt-models:/data/models \
  notare-stt-server:cpu
```

**The Dockerfile is untested** — there was no Docker available in the
environment this was built in. It's a standard two-stage `rust:bookworm` ->
`debian:bookworm-slim` build with the same apt packages the Linux desktop CI
build installs for whisper-rs/bindgen (`cmake`, `clang`, `libclang-dev`,
`build-essential`). Build and smoke-test it before relying on it in
production; `Dockerfile.vulkan` / `Dockerfile.cuda` are Phase 4.

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
