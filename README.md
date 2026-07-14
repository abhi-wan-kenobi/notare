# Notare

**Open-source, local-first meeting notetaker + dictation for macOS, Windows, and Linux.**

One app, two modes:

- **Meeting mode** — captures system + microphone audio locally (no bots join your
  calls), transcribes with a pluggable STT backend, and *enhances your own rough
  notes* with the transcript as context. Notes land as plain Markdown in a folder
  you choose (e.g. an Obsidian vault).
- **Dictation mode** — push-to-talk anywhere: speak, and the text is typed into
  whatever app you're using.

Plus a **companion STT server** (Docker, NVIDIA CUDA + AMD Vulkan) exposing an
OpenAI-compatible `/v1/audio/transcriptions` API with a web page for model
management — point the desktop app at it and any GPU box on your LAN does the
heavy lifting.

## Principles

- **Local-first, privacy-first.** Audio and transcripts never leave your machine
  unless *you* configure a remote backend. No telemetry by default. No accounts.
- **BYO everything.** Your own STT models (HuggingFace), your own LLM endpoint
  (Ollama / OpenAI-compatible / Anthropic), your own Google OAuth client for
  calendar. No hosted service, no vendor lock-in.
- **Plain Markdown output.** Your notes vault is the database.
- **Never trust a flag over the filesystem.** Model state is reconciled against
  on-disk reality (sha256-verified) on every launch.

## Status

🚧 **Pre-alpha — planning phase.** See [`docs/PROJECT-BRIEF.md`](docs/PROJECT-BRIEF.md)
for the full brief and [`docs/adr/`](docs/adr/) for architecture decisions.

## License

TBD pending upstream license diligence (see `docs/`); this will be a copyleft
license if the app core is derived from [Hyprnote](https://github.com/fastrepl/hyprnote).
