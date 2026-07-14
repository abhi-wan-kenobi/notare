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
  unless *you* configure a remote backend. No telemetry, period — the analytics
  backend is compiled out. No accounts.
- **BYO everything.** Your own STT models (HuggingFace), your own LLM endpoint
  (Ollama / OpenAI-compatible / Anthropic), your own Google OAuth client for
  calendar. No hosted service, no vendor lock-in.
- **Plain Markdown output.** Your notes vault is the database.
- **Never trust a flag over the filesystem.** Model state is reconciled against
  on-disk reality (sha256-verified) on every launch.

## Status

🚧 **Pre-alpha.** The codebase was just forked from
[anarlog](https://github.com/fastrepl/anarlog) (fork point `c92cbbadf`,
2026-07-14) and is being reshaped. See [`docs/PROJECT-BRIEF.md`](docs/PROJECT-BRIEF.md)
for the full brief and [`docs/adr/`](docs/adr/) for architecture decisions.

Headline goals over upstream: **shipped Windows and Linux support**, a unified
**dictation mode**, a rebuilt self-healing **model manager**, and the
**companion GPU STT server**.

## Lineage & credits

Notare is a friendly MIT fork of **[anarlog](https://github.com/fastrepl/anarlog)**
(which started life as **Hyprnote**) by [Fastrepl](https://github.com/fastrepl) —
an excellent local-first notetaker whose audio-capture and session core this
project builds on. Dictation-mode inspiration:
[OpenWhispr](https://github.com/OpenWhispr/openwhispr) and
[Handy](https://github.com/cjpais/Handy).

## License

[MIT](LICENSE) — including upstream anarlog code (MIT, Fastrepl, Inc.).
See [`docs/LICENSE-NOTE.md`](docs/LICENSE-NOTE.md) for full license diligence,
including per-model licenses in the STT catalog.
