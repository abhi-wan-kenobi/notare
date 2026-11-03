# License & Legal Diligence Note

_Status: final 2026-07-14; extended 2026-08-28; re-opened 2026-09-02; settled 2026-09-07 and resolved in 0.6.6 / 0.6.7 by migrating to cr-sqlite (MIT) and deleting `crates/cloudsync` (see "Sync engine" below). All licenses verified from the actual repos & HF model cards, not from memory._

## Sync engine: sqlite-sync (Elastic License 2.0) — RESOLVED: MIGRATED TO CR-SQLITE (2026-09-07 / 0.6.7)

> **Resolution (2026-09-07 / 2026-11-04)**: Following SQLite Cloud's written denial of 2026-09-02 refusing a licence for our custom P2P network layer, Notare abandoned sqlite-sync and migrated to **cr-sqlite** (vlcn-io v0.16.3, MIT) and Protocol v2 in release 0.6.6. In release 0.6.7, the legacy vendored tree under `crates/cloudsync/` was completely removed. This section preserves the historical record of the ELv2 diligence and the refusal that led to the engine swap.

During early v0.6 P2P sync development, sync was prototyped on **sqlite-sync v1.0.12** (SQLite Cloud), historically vendored at `crates/cloudsync/vendor/` (upstream SHA `6694c2e8`). Unlike everything else in this document it was **not** an OSI-approved licence: it was **Elastic License 2.0** (ELv2) — source-available, with the headline restriction that you may not "provide the software to third parties as a hosted or managed service."

**Why this needed a determination.** Notare is MIT. Embedding the extension, building it from source (which was required for the custom P2P network layer — see `docs/internal/archive/sync-p2p-cloudsync-era.md`), and replacing the upstream transport entirely went beyond plain redistribution. It was treated as a blocking gate (**risk R1**, the only cycle-killing risk in the v0.6 plan) rather than assumed.

### Timeline

| Date | Event |
|---|---|
| 2026-08-28 | Recorded here as **PERMITTED**, from a summary of a conversation with SQLite Cloud. Flagged at the time as resting on a second-hand source. |
| 2026-09-02 | **Marco Bambini (SQLite Cloud) replied in writing:** *"The short answer is, unfortunately, no, you cannot use your network layer with our technology."* |
| 2026-09-07 | Swap to cr-sqlite (vlcn-io v0.16.3, MIT) and protocol v2 approved. Implemented in 0.6.6; legacy `crates/cloudsync` deleted in 0.6.7. |

**Final status: RESOLVED — sqlite-sync abandoned; migrated to cr-sqlite (MIT).** The denial confirmed that SQLite Cloud would not permit our custom network layer (`CLOUDSYNC_OMIT_CURL` + `network_send_buffer` / `network_receive_buffer` in `network_p2p.c`). Because that integration was the foundation of the P2P transport, the refusal stopped all use of sqlite-sync.

**Consequences & Outcome:**

- **sqlite-sync was abandoned as the CRDT engine.** Sync was deferred from 0.6 to 0.7 (`docs/release/0.7-sync-gate.md`) while the swap to cr-sqlite (vlcn-io v0.16.3, MIT) was completed in 0.6.6.
- **cr-sqlite (vlcn-io, MIT) replaced sqlite-sync.** All synced tables (`crates/db-app`) were validated against cr-sqlite's compatibility rules and STRICT schema requirements (`crates/crsqlite/tests/strict_roundtrip.rs`, `docs/internal/sync-p2p.md §30`).
- **Complete removal in 0.6.7:** As decided in step 3 of the original contingency plan, the entire vendored tree under `crates/cloudsync/`, the custom C network layer (`network_p2p.c`), the `from-source` build feature, and the `cloudsync_from_source_*` CI jobs were deleted from the repository in release 0.6.7.
- Notare's sync stack is now entirely under permissive licensing (cr-sqlite is MIT; `crates/sync-p2p` and Notare are MIT). No ELv2 or proprietary restrictions remain.
**Process lesson worth keeping:** this note recorded a blocking legal gate as
cleared on a second-hand summary, and a full release cycle (~75 of 115 commits since
v0.5.2) was built on it. A gate that can kill a cycle needs its primary source
archived *before* it is marked cleared, not as a follow-up item.

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
