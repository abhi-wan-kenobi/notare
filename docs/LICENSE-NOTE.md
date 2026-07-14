# License & Legal Diligence Note

_Status: draft 2026-07-14. STT engines/models verified from repos & HF model cards;
Hyprnote (upstream fork target) section pending final verification._

## Upstream: Hyprnote

**PENDING — this section gates the project license.** If Hyprnote is GPL/AGPL, any
fork derived from its code must remain under that (or a compatible copyleft)
license; MIT/Apache re-licensing is impossible. If it carries an `ee/` (enterprise)
directory under a commercial license, that code must be excluded from the fork.

## STT engines & libraries (all verified 2026-07-14)

| Component | License | Copyleft | Notes |
|---|---|---|---|
| whisper.cpp (ggml-org) | MIT | No | Keep copyright notice. Vulkan backend is first-class (`GGML_VULKAN=1`). |
| faster-whisper (SYSTRAN) | MIT | No | CUDA via CTranslate2. |
| CTranslate2 (OpenNMT) | MIT | No | CUDA + CPU; ROCm wheels now on releases page. No Vulkan. |
| sherpa-onnx (k2-fsa) | Apache-2.0 | No | Whisper/Zipformer/Parakeet runtimes; CUDA primary. |
| Speaches (ex faster-whisper-server) | MIT | No | OpenAI-compatible `/v1/audio/transcriptions` confirmed. Pre-1.0. CUDA-only officially. |

## Models

| Model | License | Gated | Commercial | Notes |
|---|---|---|---|---|
| openai/whisper-large-v3 | Apache-2.0 (HF card) | No | Yes | GitHub repo says MIT; both permissive. |
| openai/whisper-large-v3-turbo | MIT | No | Yes | Recommended default (speed/quality). |
| distil-whisper/distil-large-v3.5 | MIT | No | Yes | Fast English option. |
| nvidia/parakeet-tdt-0.6b-v2/v3 | CC-BY-4.0 | No | Yes | **Attribution required.** v2 English-only; v3 = 25 European languages, no Indic. |
| mistralai/Voxtral-Mini-3B | Apache-2.0 | No | Yes | 8 languages **incl. Hindi** — best new option for Indian English. |
| nvidia/canary-qwen-2.5b | CC-BY-4.0 | No | Yes | English-only, top of Open ASR leaderboard. |
| kyutai/stt-2.6b-en | CC-BY-4.0 | No | Yes | Streaming STT, English. |
| nvidia/canary-1b (original) | CC-BY-**NC**-4.0 | No | **NO** | Non-commercial — **exclude from catalog** (v2 reportedly CC-BY-4.0, verify before adding). |
| ai4bharat/indic-conformer-600m | MIT (reported) | No | Yes | 22 Indian languages; verify license on HF card before catalog inclusion. |

## Implications

1. **The entire STT stack is permissive (MIT/Apache/CC-BY).** Nothing below the app
   layer constrains our license choice.
2. **The project license is determined solely by Hyprnote's license** (fork target).
3. CC-BY models (Parakeet, Canary, Kyutai) require visible attribution in the model
   catalog UI/docs — the catalog JSON should carry a `license` + `attribution` field
   per model and the UI should display it.
4. Never add CC-BY-NC or research-only models to the default catalog. Catalog PRs
   must include license verification.

## AMD (RX 6600 / RDNA2) reality check

- whisper.cpp Vulkan works cross-vendor and is the pragmatic AMD server path, but
  RDNA2 has reported **silent CPU-fallback** cases — the companion server must
  log/verify GPU offload at startup and expose it in the web admin page (a
  "backend actually using GPU: yes/no" indicator), not just assume it.
- Alternative AMD path: whisper.cpp ROCm/hipBLAS build.
- faster-whisper/Speaches remains the NVIDIA (CUDA) path.
