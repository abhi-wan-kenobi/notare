# LLM-EMBED — restore the in-process local LLM server on llama.cpp (authoritative spec)

**Status: QUEUED, not started.** Do not begin this until the v0.6 sync cycle
(SYNC-8 + SYNC-9) is merged. Both were mid-flight when this was written.

**This file is a working spec — do NOT `git add` it or include it in any
commit.** Leave it untracked.

Where this spec conflicts with what you find in the code, **STOP and report the
conflict rather than improvising.**

## Goal

notare's "intelligence" features currently require the user to point at an
external endpoint — `apps/desktop/src/services/llm-router/index.ts:94` defines
`LOCAL_PROVIDER_IDS = ["ollama", "lmstudio"]`, discovered by probing. That means
installing and running a second application to get local AI. This task restores
a **built-in** local LLM so notare works out of the box with no external server.

This is a *restoration*, not a greenfield build. Read the archaeology below
before designing anything.

## Archaeology — read this first, it defines the shape

Local LLM inference used to exist and was removed wholesale in commit
`dff35f223` — "Remove Cactus and reduce startup network calls" (#5505,
2026-06-08). That PR deleted a vendored inference engine (`crates/cactus`,
`crates/cactus-model`, `crates/api-cactus`, `.github/workflows/cactus.yaml`) and
everything that depended on it. **The LLM server was collateral damage: it was
removed because its engine vendor was dropped, not because the design was
wrong.**

Recover the deleted implementation and read it before writing code:

```
git show dff35f223 -- crates/local-llm-core/src/server.rs
```

It was an **in-process axum HTTP server**: bind `TcpListener` on
`(Ipv4Addr::LOCALHOST, 0)`, expose `http://<addr>/v1` (OpenAI-compatible),
graceful shutdown over a `tokio::sync::watch` channel, all gated
`#[cfg(target_arch = "aarch64")]` — an Apple-Silicon limitation of Cactus, not
of the design.

**What still exists and must not be rebuilt:**
- `crates/local-llm-core/src/server.rs` — the stub. Public API is intact:
  `start_with_model_path(name, file_path)`, `url()`, `exit_receiver()`, `stop()`.
  Every method currently errors or is `unreachable!()`.
- `plugins/local-llm` — the whole plugin survives. `src/lib.rs:28` still holds
  `Option<LlmServer>`; `src/ext.rs:103` still returns `server.url()`. Model
  download, listing, deletion, custom models all work.
- `plugins/local-llm/src/ext.rs:172` — `pub fn start_server(&self) {}`. An empty
  no-op. This is the seam.
- `hypr-gguf`, `hypr-local-model`, and the shared downloader.

**What is missing:** the engine, and any frontend wiring (`grep` finds no
reference to `@hypr/plugin-local-llm` anywhere under `apps/desktop/src/`).

## Requirement 1 — the engine: `llama-cpp-2`, already in the tree

Do **not** add a new inference dependency. `llama-cpp-2` v0.1.151 is already a
workspace dependency at `crates/transcribe-voxtral-llama/Cargo.toml:22` (with
`mtmd`), used for Voxtral **speech** transcription behind the opt-in
`voxtral-llama` feature on `plugins/local-stt`.

`crates/transcribe-voxtral-llama/src/model.rs` is a working in-repo reference for
`LlamaBackend`, `LlamaContext`, `LlamaContextParams`, `LlamaBatch` and
`LlamaModelParams`. Read it before writing your own. Note especially how it
handles backend initialisation lifetime — `LlamaBackend` is process-global and
initialising it twice is a known footgun; the STT path already solves this and
your server must not conflict with it if both are enabled at once. **Verify what
happens when local-stt's Voxtral and this LLM server are both active. If they
cannot coexist, STOP and report — do not silently make one exclude the other.**

Implement `LlmServer` keeping the **existing public API unchanged**, so
`plugins/local-llm` needs no signature changes.

## Requirement 2 — serve OpenAI-compatible `/v1`

Restore the axum server shape from `dff35f223`: bind localhost port 0, serve
`/v1/chat/completions` including **SSE streaming**, graceful shutdown, and
`url()` returning the base URL. This is what makes it drop-in for the existing
router, which already speaks OpenAI-compatible HTTP to local providers.

Loopback-only. Never bind a routable interface.

## Requirement 3 — structured output must not regress

`apps/desktop/src/services/action-items/structured-capability.ts` exempts Ollama
from capability probing because Ollama's native `format` endpoint guarantees
JSON; other providers must pass a probe or they are refused (that refusal is a
production gate). An embedded server has to satisfy this or action-item
extraction silently degrades.

`llama-cpp-2` exposes an **`llguidance`** feature (grammar-constrained
generation) — verified present in the crate's `[features]`. Use grammar
constraints to guarantee schema-valid JSON, then either pass the existing probe
honestly or add an exemption **justified by the grammar guarantee**, not by
convenience. Cover it with a test in `structured-capability.test.ts` alongside
the existing Ollama cases.

## Requirement 4 — wire the router and the plugin seam

- Un-stub `start_server()` in `plugins/local-llm/src/ext.rs:172`; make
  `server_url()` return the live URL.
- Register the embedded server as a provider in
  `apps/desktop/src/services/llm-router/`. Discovery should read `server_url()`
  from the plugin, **not** probe a port — it is in-process and its address is
  known.
- **Fail closed.** If the embedded provider is selected and unavailable, surface
  a clear error. Do NOT silently fall back to a cloud provider. (OpenWhispr
  shipped this exact fix in 1.8.3 — "custom endpoints fail closed, no OpenAI
  fallback". For a local-first notes app, a silent cloud failover is a privacy
  incident, not a graceful degradation.)
- Follow `AGENTS.md`: tanstack-query for state, `cn` from `@hypr/utils` with an
  array, `motion/react`, i18n every user-facing string.

## Requirement 5 — feature-gate and platform support, proven not claimed

`llama-cpp-2` builds C++ and is expensive. Follow the `plugins/local-stt`
precedent exactly: opt-in cargo features, **default OFF**, so a default build
and today's CI are byte-identical in behavior.

GPU backends available on `llama-cpp-2` 0.1.151, verified from its `[features]`:
`cuda`, `vulkan`, `rocm`, `metal`, `opencl`, `mkl`. Mirror the STT naming
(`voxtral-llama` / `voxtral-llama-cuda`), e.g. `local-llm`, `local-llm-cuda`,
`local-llm-vulkan`. Vulkan matters — it is the path for AMD/Intel GPUs.

**Every platform you claim must have a CI job that compiles it.** This is not
negotiable and is not theoretical: SYNC-9 widened a `supported()` predicate on
reasoning alone and its first real CI run failed instantly with undefined
`_SecRandomCopyBytes` on macOS. Reasoning about a native build is not evidence.
A `cargo check` job per claimed platform, cheap, no full app build.

## Requirement 6 — models: download, don't bundle

Reuse the existing download scaffolding (`hypr-gguf`, `is_model_downloaded`,
the shared downloader). Do **not** vendor weights into the repo or installer.
Add **SHA-256 verification** of downloaded weights (OpenWhispr does this for its
GPU packs; weights deserve the same).

The current registry needs attention — check it rather than trusting it:
`crates/local-llm-core/src/model.rs` gates `SUPPORTED_MODELS` to
`#[cfg(target_arch = "aarch64")]` and it is **empty on every other arch**, while
`list_supported_models()` unconditionally returns three. Two of those three
(`Gemma3_4bQ4`, `Llama3p2_3bQ4`) are self-described as "Deprecated. Exists only
for backward compatibility." Decide the real shipping set deliberately and say
why in the report; do not just widen the cfg and inherit deprecated entries.

## Out of scope — do NOT do

- Removing or breaking the existing `ollama` / `lmstudio` providers. They stay;
  this is an additional option, not a replacement.
- Touching the Voxtral STT path beyond what coexistence requires.
- Bundling model weights.
- Any v0.6 sync work.

## Open questions to resolve during implementation (report your answers)

1. **In-process memory pressure.** Cactus was `aarch64`-only. Determine whether
   that was a Cactus packaging limit or a deliberate choice about running
   inference inside the app process. If in-process memory is the real risk, a
   bundled `llama-server` subprocess is the alternative shape (it is what
   OpenWhispr ships) — but it costs a binary to package per platform and does
   not fit the surviving `LlmServer` API. Recommend one, with reasoning.
2. **Backend coexistence** with Voxtral STT (Requirement 1).
3. **Binary size and build time** impact with the feature on, per platform.

## Verification bar

1. `cargo check -p desktop` default AND with the new feature — both green.
2. `cargo test` for `local-llm-core` and `tauri-plugin-local-llm`, both configs.
3. `pnpm -F desktop typecheck`, `pnpm -r typecheck`, desktop vitest suite — the
   baseline is 266 files / 2107 tests passing; anything less is a regression.
4. `pnpm exec dprint fmt`, then `dprint check` clean on changed files.
5. A real end-to-end run: download a model, start the server, complete a chat
   request, and extract action items through the router with the embedded
   provider selected. Report actual output, not "it should work".
6. `cargo check --workspace --all-targets` CANNOT be green on Linux
   (`crates/shortcut-macos`, `E0455`). Pre-existing; do not chase it.
7. Run the `auditor` skill, `--coder claude-sonnet-5`, ONE commit or file at a
   time — large payloads silently return empty reports that read as clean.
   Verify each finding against the real code; roughly a third are false
   positives.

## Report when done

Commits (hash + subject); the engine wiring and how it coexists with Voxtral;
your answers to the three open questions; the model set you chose and why; which
platforms have CI proof and which do not; verbatim verification output; and the
auditor summary split into confirmed-fixed / false-positive-with-why /
carried-forward.
