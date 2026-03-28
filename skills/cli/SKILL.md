---
name: char-cli
description: Live transcription, recording, and audio tools CLI. Use when the user needs to transcribe audio, record, play audio files, or manage speech-to-text models.
---

# char CLI

char is a CLI for live transcription and audio tools.

## Commands

### Transcribe audio

```bash
char transcribe --input <file>
```

Transcribe an audio file. Supports WAV, MP3, and other common formats.

Options:
- `--input <file>` — audio file to transcribe (required)
- `--provider <name>` — STT provider (e.g. `whispercpp`, `cactus`)
- `--language <code>` — language hint (ISO 639-1)
- `--model <model>` — model name to use
- `--format pretty|json` — output format (default: `pretty`)

### Record audio

```bash
char record
```

Record audio from the microphone to an MP3 file.

Options:
- `-o, --output <file>` — output file path
- `--sample-rate <hz>` — sample rate (default: 16000)

### Play audio

```bash
char play <file>
```

Play an audio file with an interactive TUI waveform display.

### Manage models

```bash
char models list
char models download <name>
char models delete <name>
```

List, download, or delete local speech-to-text models.

## Tips

- Use `char transcribe --format json` for machine-readable output (JSONL events).
- Use `char models list --format json` to check available models programmatically.
- All commands support `--verbose` / `-v` flags for debug logging.
