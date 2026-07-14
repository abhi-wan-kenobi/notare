# License & Legal Diligence Note

_Status: final 2026-07-14. All licenses verified from the actual repos & HF model
cards, not from memory._

## Upstream: anarlog (formerly Hyprnote) — VERIFIED

- **Repo:** `fastrepl/hyprnote` was renamed **Hyprnote → char (2026-02) → anarlog
  (2026-04)**; `github.com/fastrepl/hyprnote` redirects to
  `github.com/fastrepl/anarlog`. Fastrepl's flagship is now **char** (separate,
  closed codebase); anarlog remains the maintained open-source app.
- **License: MIT** — single root LICENSE, "Copyright (c) 2023-present Fastrepl,
  Inc.", whole repo. **Relicensed from GPL-3.0 to MIT on 2026-04-26**
  ([PR #5132](https://github.com/fastrepl/anarlog/pull/5132)). No `ee/` folder, no
  dual licensing, no CLA.
- **Caveats:**
  1. **Fork from current `main` (or any post-2026-04-26 tag) only.** Anything
     cherry-picked from pre-relicense history is GPL-3.0.
  2. Contributor consent to the GPL→MIT relicense is not publicly documented;
     Fastrepl holds the copyright notice. Residual risk accepted, noted here.
  3. **Trademarks/names:** "Hyprnote" and "char" belong to Fastrepl (hyprnote.com
     → char.com). The fork must not use those names in branding; internal `hypr-`
     crate prefixes should be renamed over time.
  4. The repo contains anarlog's **cloud backend** (`apps/api`, `apps/stripe`,
     `supabase/`, auth/subscription crates) for their hosted Pro tier — **strip
     from the fork**.

## Our license: MIT (decided 2026-07-14)

MIT, matching upstream — simplest attribution story, maximum adoption. The root
LICENSE carries our copyright plus the Fastrepl attribution line for derived
portions.

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

1. **The entire stack — upstream app AND STT layer — is permissive.** No copyleft
   obligations anywhere.
2. Keep upstream's MIT copyright notice alongside ours (done in root LICENSE).
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
