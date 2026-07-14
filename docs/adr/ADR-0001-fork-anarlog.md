# ADR-0001: Fork anarlog (ex-Hyprnote) as the app core; license MIT

Date: 2026-07-14 · Status: **Accepted**

## Context

Notare replaces three tools (Anarlog/Granola-class meeting notetaker + OpenWhispr
dictation) with one local-first, cross-platform app. Candidates for the starting
codebase: fork anarlog, fork OpenWhispr, or greenfield.

Research findings (2026-07-14, see LICENSE-NOTE.md for sources):

- **anarlog** (fastrepl/anarlog, 8.8k stars, daily releases): Tauri v2 + Rust
  workspace (~180 crates) + React/Vite frontend. Solves the hard problems already:
  per-platform audio capture crates (macOS Core Audio taps via `cidre`, Windows
  `wasapi`, Linux PulseAudio/PipeWire), AEC/denoise/AGC/VAD, session management,
  whisper.cpp via `whisper-rs` (metal/coreml/cuda/vulkan features), model
  downloader, a dictation/push-to-talk plugin, and an OpenAI-compatible STT client.
  MIT since 2026-04-26. **But upstream ships macOS-only** — Windows/Linux builds
  were promised and then pulled (May 2026); the cross-platform code paths are
  scaffolding, not shipped product.
- **OpenWhispr**: MIT, Electron, dictation-only — no capture stack, wrong chassis.
- Feasibility research confirms Windows (WASAPI loopback) and Linux (monitor
  sources) capture are proven in other shipping Rust/Tauri apps (Meetily, Vibe,
  Pluely). The only genuinely risky surface is dictation on Linux/Wayland.

## Decision

1. **Fork anarlog at current `main`** (post-relicense — never cherry-pick
   pre-2026-04-26 GPL history).
2. **License Notare as MIT**, retaining Fastrepl's copyright notice for derived
   portions. Avoid "Hyprnote"/"char" naming; rename `hypr-` prefixes over time.
3. **Strip the cloud backend** (`apps/api`, `apps/stripe`, `supabase/`,
   auth/subscription crates) — Notare has no hosted tier, no accounts.
4. **Own the cross-platform gap**: finishing Windows and Linux support is Notare's
   headline contribution, building on the in-tree scaffolding. Windows loopback
   via the `wasapi` crate (not cpal), Linux via Pulse monitor / `pipewire-rs`.
5. Rebuild the model manager with **filesystem-reconciled state** (sha256-verified
   on every launch — never trust a stored flag) and a licensed model catalog.
6. Keep STT and LLM pluggable behind OpenAI-compatible interfaces; ship a
   companion Docker STT server (CUDA + Vulkan) with a web admin page.

## Consequences

- We inherit a large, fast-moving upstream — early on we can rebase/merge to pick
  up fixes; divergence will grow and merges will eventually stop being practical.
- Electron-free (Tauri), one Rust workspace to learn — steep initial ramp.
- Windows/Linux support is real engineering, not configuration; Wayland dictation
  ships as tier-2 (portal + injection cascade, clipboard-paste fallback).
- MIT permits closed forks — accepted trade-off for adoption simplicity.
