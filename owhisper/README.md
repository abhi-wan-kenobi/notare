# OWhisper

> Something like Ollama, but for realtime speech-to-text.

## Get started

### Homebrew

```bash
# install owhisper CLI
brew tap fastrepl/hyprnote && brew install owhisper

# download example config
curl -O ./config.json https://raw.githubusercontent.com/fastrepl/hyprnote/refs/heads/main/owhisper/owhisper-config/examples/cli.json

# serve it with owhisper
owhisper serve --config ./config.json --port 8080
```

### Docker
```bash
# download model for whisper.cpp
curl -O ./ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin?download=true

# download example config
curl -O ./config.json https://raw.githubusercontent.com/fastrepl/hyprnote/refs/heads/main/owhisper/owhisper-config/examples/docker.json

# serve it with owhisper
docker run -p 8888:8888 \
  -v $(pwd)/config.json:/app/assets/config.json:ro \
  -v $(pwd)/ggml-base.bin:/app/assets/ggml-base.bin:ro \
  ghcr.io/fastrepl/owhisper:latest \
  serve --config /app/assets/config.json --port 8888
```
