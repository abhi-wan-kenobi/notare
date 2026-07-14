# ADR-0002: Telemetry removal, cloud-client neutering, model integrity, Linux STT

Date: 2026-07-14 · Status: Accepted

## 1. Telemetry: compiled out, not opted out

Upstream ships PostHog analytics on by default (opt-out flag in a store) plus
optional Sentry. Notare's principle is "no telemetry, period", so the kill is
at the single Rust choke point both the analytics plugin and the feature-flag
plugin construct their client through: `AnalyticsClientBuilder::build()`
(`crates/analytics`) now always returns a client with no PostHog backend.
Every send/flag path already no-ops (local tracing only) when the backend is
absent, so the ~32 frontend call sites and the Tauri command surface keep
working unchanged. Sentry remains env-gated (`SENTRY_DSN`); our builds carry
no DSN. The release-build `assert!` requiring a PostHog key was removed so
Notare compiles without any vendor secrets.

## 2. Cloud client: neutered now, excised later

The upstream cloud (api.anarlog.so, Supabase auth, Stripe) is unreachable by
design in our builds: no `VITE_SUPABASE_*` env → null auth client → every
cloud call is session-gated and can't fire. Release builds no longer require
`VITE_API_URL` (plugins/calendar falls back to localhost). Changelog fetch
repointed from fastrepl's repo to ours. Remaining outbound surface: GitHub
(updater/changelog) and the upstream S3 bucket for model downloads (public,
unauthenticated; mirroring is planned with the companion server phase).
Full deletion of `plugins/auth`, `crates/api-client`, sign-in/Pro UI is
deferred to the meeting-mode UX phase — `crates/cloudsync` must stay (the
local DB loads it even with network sync off).

## 3. Model integrity: never trust a flag over the filesystem

The bug class this project was founded on: stored state claimed a model was
installed while the files were gone/corrupt, and transcription failed forever.
Design:

- **Status is always derived from disk.** `is_downloaded` for single-file
  models now requires exact expected size, not bare existence.
- **Startup reconciliation** (both STT and LLM plugins): every catalog model
  is verified (existence → size → CRC32) on launch. Corrupt files are
  quarantined (`*.corrupt`) so status queries report "not installed", and a
  Failed download event is emitted so the UI offers re-download.
- **Verification stamps:** a `<file>.verified` sidecar (size+mtime+checksum)
  caches a successful full hash so multi-GB models aren't re-hashed every
  launch; any file change invalidates it.
- **Pre-load guard:** starting the internal whisper server now checks the
  model is actually installed (previously only external servers did), so a
  missing model surfaces as `ModelNotDownloaded` instead of an opaque engine
  failure.

Checksums are CRC32 because those are the authoritative values upstream
publishes for its S3 artifacts — we do not invent sha256 values we cannot
verify. When Notare hosts its own model catalog (companion-server phase),
the catalog gains sha256 per file and the verifier upgrades.

## 4. Local STT on Linux/Windows

Upstream compiled the whisper.cpp engine into no build (macOS uses the
Apple-Silicon-only Argmax/Soniqo runtimes). Notare enables the plugin's
`whisper-cpp` feature for Linux and Windows targets (CPU inference; GPU
features come with the STT-server phase), marks Whisper models available on
all desktop platforms, and adds the 7 quantized Whisper variants to the
supported-models catalog. The catalog list command hides Whisper entries on
builds without the engine, so macOS behavior is unchanged.
