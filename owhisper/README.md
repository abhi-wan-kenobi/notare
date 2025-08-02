# OWhisper

> Something like Ollama, but for realtime speech-to-text.

## Installation

### Homebrew

```bash
brew tap fastrepl/hyprnote && brew install owhisper
```

### Docker

```bash
docker run -p 8888:8888 \
  -v $(pwd)/config.docker.example.json:/app/assets/config.yaml:ro \
  -v $(pwd)/tiny-en.gguf:/app/assets/tiny-en.gguf:ro \
  ghcr.io/fastrepl/owhisper:latest \
  serve --config-path /app/assets/config.json --port 8888
```

## Usage

```bash
owhisper serve --config ./config.yaml --port 8080
```
