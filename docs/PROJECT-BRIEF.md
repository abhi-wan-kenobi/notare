# PROJECT BRIEF: Notare — open-source, cross-platform meeting notetaker + dictation

## Role of the implementing agent
You are the lead engineer + architect. Before writing ANY code, produce (1) a
license/legal diligence note, (2) an architecture decision record, and (3) a
phased implementation plan for review. Do not start coding until the plan is
approved.

## Goal
Fork **anarlog** (formerly Hyprnote — Tauri v2 + Rust + React) and evolve it into a
single, polished, open-source app for **macOS, Windows, and Linux** that replaces
three tools:
- **Anarlog / Granola-class notetakers** — meeting capture (system+mic audio →
  transcript → AI-enhanced notes)
- **OpenWhispr** — push-to-talk dictation (speak → text inserted into the active app)

One app, two modes: **Meeting mode** and **Dictation mode**.

## Why fork anarlog (not OpenWhispr) — see ADR-0001
The hard, fragile part of a notetaker is **simultaneous system+mic audio capture**
(Core Audio taps on macOS, WASAPI loopback on Windows, PipeWire/Pulse monitor
sources on Linux) plus session management and calendar linking. Anarlog ships this
on macOS and carries real scaffolding for Windows/Linux (wasapi + Pulse/PipeWire
crates in-tree), plus AEC/denoise/VAD, a dictation plugin, and an OpenAI-compatible
STT client. OpenWhispr is dictation-only (Electron) and would mean rebuilding
capture from scratch. STT itself is the EASY, swappable part. So: keep anarlog's
capture + session core; **finish Windows/Linux support (upstream ships macOS-only
today — this is our headline contribution)**; rip out and rebuild the model/STT
layer and the UX; strip their cloud backend (api/stripe/supabase).

## Hard lesson to design around (real bug from the app being replaced)
The current app's stored state claimed the STT model was "downloaded" while the model
files were actually absent on disk (leftover from an uninstall). Recording worked,
transcription silently failed forever, and restarts never self-healed because the app
trusted its own flag over reality.
**Requirement:** the model manager MUST reconcile declared state against the
filesystem on every launch — verify each model's files exist AND match expected
sha256; if not, mark it "not installed" and surface a re-download action. Never trust
a boolean flag.

## Product vision (steal the best of Granola)
- **Notepad-first workflow:** the user types rough notes during the meeting; the AI
  *enhances* those notes using the transcript as context — rather than dumping a
  generic summary.
- **Automatic meeting detection:** calendar events AND/OR a running meeting app
  (Zoom/Meet/Teams) + active audio → prompt "Start capturing?"
- **No bots join calls** — everything captured locally, privacy-first.
- **Local-first storage in plain Markdown:** notes/transcripts land in a
  user-chosen folder (e.g. an Obsidian vault) as portable Markdown + sidecar
  files. The vault IS the database for notes; no proprietary formats.
- **Beautiful, minimal overlays:** a small floating widget during meetings (live
  status, timer, quick-note, stop) and a clean dictation HUD. Non-intrusive,
  native-feeling per OS.
- **Template-driven summaries** (standup, 1:1, client call, etc.), user-editable.

## STT architecture — pluggable backends (the core redesign)
Define a single `TranscriptionBackend` interface (batch + streaming). Ship these
implementations, selectable per-platform in settings:
1. **Local — Apple Silicon:** whisper.cpp (Metal) and/or Parakeet via
   sherpa-onnx / mlx-whisper.
2. **Local — Windows/NVIDIA:** faster-whisper (CTranslate2, CUDA) for best
   quality/speed.
3. **Local — Windows/AMD & Linux:** whisper.cpp with the **Vulkan** backend
   (CUDA engines don't support AMD).
4. **Remote:** any endpoint speaking the **OpenAI-compatible
   `/v1/audio/transcriptions`** API — see "Companion STT server" below.

**Recommended default models:** whisper-large-v3 (best multilingual / accented
English, incl. Indian English), whisper-large-v3-turbo (faster), Parakeet (fast
English). User picks per mode (dictation may use a smaller/faster model than
meetings).

### Model download manager (get this right)
- A **model catalog** (JSON): id, display name, HF repo id, files, sizes, sha256,
  supported backends, language notes.
- Downloads from **HuggingFace Hub** (optional user HF token for gated repos).
- Resumable, progress-reported, atomic (temp download → verify sha256 → atomic
  rename). Clean up partials on failure.
- **Startup reconciliation** (see "hard lesson"): verify on-disk reality, self-heal.
- Clear UI: per-model state (not installed / downloading / verifying / ready /
  corrupt), disk usage, delete, re-download.

### Companion STT server (Docker-first)
A separate deployable exposing the OpenAI-compatible transcription API, so the
desktop app just points at a URL:
- **Docker images for BOTH GPU vendors:** NVIDIA (CUDA — faster-whisper/Speaches)
  and AMD (Vulkan via whisper.cpp — e.g. RX 6600; ROCm optional where supported).
  Backend chosen via config; document AMD/Vulkan setup explicitly.
- Batch endpoint + optional streaming (WebSocket) for live captions.
- **Built-in web admin page:** small frontend for managing models (catalog browse,
  download w/ progress, verify, delete, disk usage) and server status (backend,
  GPU, loaded model, recent requests). Same catalog + reconciliation rules as the
  desktop app.
- Auth via simple bearer token. Meant for LAN / Tailscale hosts.
- `docker compose up` and go; bare-metal path documented too.

## LLM layer (summaries / note enhancement) — pluggable, BYO
- Providers: **Ollama** (local), OpenAI, Anthropic, any OpenAI-compatible endpoint.
- User supplies base URL + API key. No hosted default, no vendor lock-in.
- Prompts/templates are local files the user can edit.

## Google auth (calendar) — BYO credentials
- Users bring their own Google OAuth client (id + secret) for Calendar access, with
  a clear setup guide (GCP project → Calendar API → OAuth client → paste).
- Secrets in the OS keychain/credential manager, never plaintext.
- Calendar is optional — the app must be fully usable without it.

## Platform support tiers (informed by feasibility research, 2026-07)
- **Meeting mode:** macOS, Windows, Linux are all tier-1 — capture is proven in
  shipping Rust/Tauri apps on all three (Hyprnote, Meetily, Vibe, Pluely).
  - Windows: `wasapi` crate for loopback (NOT cpal — its loopback support was
    removed), cpal for mic; resample/drift-compensate (`rubato`), AEC available
    (`aec-rs`/`aec3`); prefer per-process loopback to reduce echo.
  - Linux: monitor-source capture via Pulse API (`@DEFAULT_MONITOR@`, works on
    PipeWire + PulseAudio) or native `pipewire-rs`.
- **Dictation mode:** macOS (Accessibility permission), Windows (SendInput), and
  Linux/X11 are tier-1. **Linux/Wayland is tier-2:** global hotkeys via
  xdg-desktop-portal GlobalShortcuts (KDE good, GNOME flaky, wlroots missing) with
  an evdev fallback; text injection via a Handy-style cascade
  (portal/libei → virtual-keyboard → ydotool → clipboard-paste). Tauri's
  global-shortcut plugin is X11-only on Linux; overlays are no-ops on Wayland —
  degrade gracefully.

## Non-functional requirements
- **Open-source philosophy:** no telemetry by default (opt-in only, clearly
  labeled); local-first; BYO keys for every external service; no phone-home.
- **License diligence FIRST:** done 2026-07-14 — see `LICENSE-NOTE.md`
  (upstream MIT since 2026-04-26; fork post-relicense `main` only; whole STT
  stack permissive; CC-BY models need visible attribution; never ship CC-BY-NC
  models in the catalog).
- **Privacy:** all audio + transcripts stay local unless the user configures a
  remote backend; data locations and deletion obvious.
- **Cross-platform parity:** document any capability gap per OS (esp. Wayland).

## Packaging & release
- Tauri bundler: macOS (.dmg, notarized), Windows (.msi/.exe, signed),
  Linux (.AppImage + .deb; Flatpak later).
- Auto-update (Tauri updater) with signed artifacts.
- CI: GitHub Actions matrix (macOS + Windows + Linux) building and publishing to
  GitHub Releases. Companion STT server published as Docker images (CUDA + Vulkan
  variants).
- Clean README, screenshots/GIFs, self-host docs, contribution guide.

## Deliverables (phased — plan approval before Phase 1)
- **Phase 0:** license/legal note, architecture decision record, phased plan, repo
  bootstrap (forked, building, renamed).
- **Phase 1:** rebuilt model manager + local STT backends (all platforms), with
  startup reconciliation. Batch transcription end-to-end.
- **Phase 2:** Meeting mode UX — capture, live transcript, notepad-first AI
  enhancement, templates, meeting detection, overlay widget, Markdown-to-vault
  storage.
- **Phase 3:** Dictation mode — global hotkey, HUD, insert-into-active-app
  (incl. the Wayland cascade).
- **Phase 4:** companion STT server — Docker (CUDA + Vulkan), OpenAI-compatible
  API, web admin page for models.
- **Phase 5:** Google Calendar (BYO OAuth), polish, packaging, CI, docs, first
  release.

## Constraints & preferences
- Prefer standard/interoperable interfaces (OpenAI-compatible STT & LLM APIs).
- Keep the desktop app thin; heavy/optional work lives in the companion server.
- Ask before any decision that's expensive to reverse (license choice, framework
  swaps, major dependency additions).
