# Notare

**Open-source, local-first meeting notetaker + push-to-talk dictation for Windows, macOS, and Linux.**

Notare records your meetings **locally** (system audio + microphone — no bots
join your calls), transcribes them live on your GPU with whisper.cpp, and uses
the transcript to *enhance the rough notes you actually typed* — notepad-first,
not summary-slop-first. Everything lands as plain Markdown in any folder you
choose, so your Obsidian vault (or any notes app) is the database.

Hold a hotkey anywhere and it's also a system-wide **dictation** tool: speak,
release, and the text is typed into whatever app has focus.

🌐 [notare.dev](https://notare.dev)

## Download

Grab the latest installer from **[GitHub Releases](https://github.com/abhi-wan-kenobi/notare/releases/latest)**:

| Platform | File |
|---|---|
| Windows 10/11 (x64) | `.msi` |
| Linux (x64) | `.AppImage` or `.deb` |
| macOS (Apple Silicon) | `.dmg` — unsigned, see note below |

Auto-updates are built in — install once and the app keeps itself current.

> **Windows SmartScreen:** builds are not (yet) code-signed, so SmartScreen
> will warn on first run. Click **More info → Run anyway**.

> **macOS Gatekeeper:** builds are unsigned (no Apple certificates yet) and
> Apple Silicon only. Open the `.dmg`, drag **Notare** to Applications, then
> on first launch **right-click the app → Open** — or run
> `xattr -cr /Applications/Notare.app`. Intel Macs: build from source.

## Highlights

| | |
|---|---|
| 🎙️ **Meeting mode** | Captures system + mic audio locally; live transcription; no bot joins your call |
| ⚡ **GPU transcription** | whisper.cpp with Vulkan (AMD/Intel/NVIDIA) — fast local STT on Windows *and* Linux |
| ⌨️ **Dictation mode** | Push-to-talk anywhere; text is typed at your cursor, with output cleanup options |
| 📝 **Notepad-first AI** | Your own notes are enhanced with the transcript as context — bring your own LLM (Ollama / OpenAI-compatible / Anthropic) |
| 📁 **Plain Markdown** | Notes written straight into any folder — point it at your Obsidian vault |
| 📅 **Calendar context** | Google Calendar via **your own** OAuth client — no shared middleman app |
| 🖥️ **Companion STT server** | Optional Docker server (NVIDIA CUDA + AMD Vulkan) with an OpenAI-compatible `/v1/audio/transcriptions` API and web model admin |
| 🔒 **Zero telemetry** | Analytics compiled out at build time; no accounts; nothing leaves your machine unless *you* configure a remote backend |
| 🪪 **MIT licensed** | Fork-friendly, forever |

## Screenshots

*TODO — coming with the next release.*

## Principles

- **Local-first, privacy-first.** Audio and transcripts never leave your machine
  unless *you* configure a remote backend. No telemetry, period. No accounts.
- **BYO everything.** Your own STT models (HuggingFace), your own LLM endpoint,
  your own Google OAuth client for calendar. No hosted service, no lock-in.
- **Plain Markdown output.** Your notes vault is the database.
- **Never trust a flag over the filesystem.** Model state is reconciled against
  on-disk reality (checksum-verified) on every launch; corrupt models are
  quarantined automatically.

## Roadmap

Development is planned in the open. See the
[Notare Roadmap board](https://github.com/users/abhi-wan-kenobi/projects/1)
for what's up next, or browse the
[v0.2 / v0.3 milestones](https://github.com/abhi-wan-kenobi/notare/milestones)
and the [issue tracker](https://github.com/abhi-wan-kenobi/notare/issues).
Feature requests and bug reports are welcome — issues labeled
[`help wanted`](https://github.com/abhi-wan-kenobi/notare/labels/help%20wanted)
are great places to jump in.

## Build from source

See [`docs/BUILDING.md`](docs/BUILDING.md). Short version: Rust + pnpm +
Tauri v2; `pnpm install && pnpm -F desktop tauri build`. Intel-Mac users must
build from source for now — released macOS builds are Apple Silicon only.

## Lineage & credits

Notare is a friendly MIT fork of **[anarlog](https://github.com/fastrepl/anarlog)**
(which started life as **Hyprnote**) by [Fastrepl](https://github.com/fastrepl) —
an excellent local-first notetaker whose audio-capture and session core this
project builds on. Headline additions over upstream: **shipped Windows and
Linux support**, a unified **dictation mode**, a rebuilt self-healing **model
manager**, and the **companion GPU STT server**.

Further inspiration: [Meetily](https://github.com/Zackriya-Solutions/meeting-minutes)
(meeting-capture UX), [Handy](https://github.com/cjpais/Handy) and
[OpenWhispr](https://github.com/OpenWhispr/openwhispr) (dictation), and the
voice-orb visualisation crowd.

More background: [`docs/PROJECT-BRIEF.md`](docs/PROJECT-BRIEF.md) and
[`docs/adr/`](docs/adr/) for architecture decisions.

## License

[MIT](LICENSE) — including upstream anarlog code (MIT, Fastrepl, Inc.).
See [`docs/LICENSE-NOTE.md`](docs/LICENSE-NOTE.md) for full license diligence,
including per-model licenses in the STT catalog.
