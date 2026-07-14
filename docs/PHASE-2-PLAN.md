# Phase 2 Plan — Meeting mode E2E on Linux/Windows + dictation everywhere

_Based on a full gap analysis of the inherited codebase, 2026-07-14._

## What upstream already gives us (verified in-tree)

- **Notepad-first AI enhancement — the Granola move — is real and complete.**
  The user's live notepad content and pre-meeting memo are first-class inputs
  (`ai-task/task-configs/enhance-transform.ts`), and the system prompt
  explicitly focuses on what the user wrote, with the transcript as context
  (`crates/template-app/src/enhance.rs` + `assets/enhance.user.md.jinja`).
- **Template-driven summaries** exist and are user-editable (SQLite via
  `templates/queries.ts`; prompt scaffolds overridable per-summary).
- **Markdown-to-Obsidian-vault storage** exists end-to-end in Rust:
  sessions land as folders with `_memo.md`, `transcript.json`, and one
  frontmattered `.md` per enhanced note under `settings.vault_base()`
  (`crates/fs-sync-core`), with Obsidian vault detection
  (`hypr_storage::ObsidianVault`). ✅ Now surfaced: the `folder-location`
  onboarding step is included in every platform's flow (it was unreachable
  upstream).
- **Linux meeting capture** uses the same code path as macOS: cpal/ALSA mic +
  PipeWire capture-sink (PulseAudio monitor fallback) + AEC
  (`crates/audio-actual/src/speaker/linux.rs`, `crates/listener-core`).
- **Mic-in-use meeting detection** works cross-platform (PulseAudio source
  events on Linux) and shows the "start capturing?" prompt.

## Backlog (priority order)

1. ~~Surface vault/folder selection in onboarding~~ — DONE (config.tsx).
   Remaining: add a Settings-screen vault picker (`set_vault_base` /
   `obsidian_vaults` commands already exist).
2. **Cross-platform global hotkey** for dictation: `plugins/shortcut` handler
   is macOS-only (`register()` → `Unsupported` elsewhere). Use
   tauri-plugin-global-shortcut on Windows (and X11), portal/evdev cascade on
   Wayland (see PROJECT-BRIEF platform tiers).
3. **Cross-platform dictation overlay + text insertion**: webview pill on the
   existing `plugins/overlay` click-through mechanism (replaces macOS-native
   `dictation-ui-macos`); text injection via enigo/SendInput on Windows,
   Handy-style cascade ending in clipboard-paste on Linux.
4. **Wire up meeting-app detection + calendar triggers**: `crates/detect`'s
   app scanners (zoom/teams/webex process detection) exist but are never
   started and drop their callback; calendar events only decorate the
   mic prompt today.
5. **Meeting overlay HUD content** for Linux/Windows (mechanism exists,
   widget content is macOS-only today).
6. **Linux capture hardening**: validate PipeWire/Pulse fallback across
   distros, improve ALSA device naming, verify AEC quality. (Needs a native
   Linux box or dual-boot — WSL has no real meeting audio.)
7. **Windows capture parity**: `MicOnly` early-returns unsupported on
   Windows; MicAndSpeaker path needs a real-machine test pass.
8. Optional: mirror templates into the vault as editable `.md`
   (currently DB-only).

## Testing reality

WSL can compile and unit-test everything, but real capture/dictation UX needs
native machines: the Windows side of this PC (tatooine) and the work MacBook
are the target test beds. The Linux capture path can be exercised on any
native Linux install later.
