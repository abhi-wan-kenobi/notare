# License & Legal Diligence Note

_Status: final 2026-07-14; extended 2026-08-28 with a sqlite-sync / ELv2
determination that was **re-opened 2026-09-02** (see "Sync engine" below). All
licenses verified from the actual repos & HF model cards, not from memory._

## Sync engine: sqlite-sync (Elastic License 2.0) — RE-OPENED 2026-09-02

> **This section previously read "CLEARED 2026-08-28 / Determination: PERMITTED".
> That determination has been withdrawn.** It rested on a second-hand summary — the
> exact evidence gap the note itself flagged — and SQLite Cloud's written reply of
> 2026-09-02 contradicts it. The prior text is preserved in git history; do not act
> on it. **Sync must not ship until this is resolved.**

The v0.6 P2P sync work is built on **sqlite-sync v1.0.12** (SQLite Cloud), vendored
at `crates/cloudsync/vendor/` (upstream SHA `6694c2e8`). Unlike everything else in
this document it is **not** an OSI-approved licence: it is **Elastic License 2.0**
(ELv2) — source-available, with the headline restriction that you may not "provide
the software to third parties as a hosted or managed service."

**Why this needed a determination.** Notare is MIT. It embeds the extension, builds
it *from source* (required for the custom P2P network layer — see
`docs/internal/sync-p2p.md`), and replaces the upstream transport entirely. That
combination is more than plain redistribution, so it was treated as a blocking gate
(**risk R1**, the only cycle-killing risk in the v0.6 plan) rather than assumed.

### Timeline

| Date | Event |
|---|---|
| 2026-08-28 | Recorded here as **PERMITTED**, from a summary of a conversation with SQLite Cloud. Flagged at the time as resting on a second-hand source. |
| 2026-09-02 | **Marco Bambini (SQLite Cloud) replied in writing:** *"The short answer is, unfortunately, no, you cannot use your network layer with our technology."* |
| 2026-09-02 | Follow-up drafted — asking whether that is an ELv2 reading or a product position, and whether a commercial licence exists. **Not yet sent.** |

**Current status: NOT PERMITTED, pending clarification.** The denial lands precisely
on the seam v0.6's sync is built on: the documented Custom Network Layer
(`CLOUDSYNC_OMIT_CURL` + `network_send_buffer` / `network_receive_buffer`),
implemented in `crates/cloudsync/build/network_p2p.c`. It is not a peripheral
restriction — it is the integration point.

**Consequences:**

- **sqlite-sync is no longer settled as the v0.6 CRDT engine.** Sync is deferred to
  0.7 (`docs/release/0.7-sync-gate.md`); 0.6 ships without it. No code change was
  needed to effect that — `release.yaml` never built the `sync` feature, so the
  stack has never reached a user.
- **cr-sqlite (vlcn-io, MIT) is live fallback scope again.** The previous "dead
  scope — do not re-plan onto it" instruction is withdrawn. A contingency scoping
  pass exists; its headline findings are that the app's six synced tables clear
  cr-sqlite's compatibility gate on paper (no FKs, no unique indices beyond the PK,
  no AUTOINCREMENT, NOT NULL columns already carry DEFAULTs), that `STRICT`-table
  behaviour is **unverified** and gates everything, and that upstream cr-sqlite has
  had no substantive work since 2024-01-17.
- ELv2 is **not** MIT. Notare's own MIT licence is unaffected, but the vendored
  tree is separately licensed and must keep its upstream LICENSE file and notices
  intact. Do not represent the bundled extension as MIT. **This holds regardless of
  how the determination resolves**, for as long as the tree is vendored.
- The restriction that must never be violated: **do not offer sqlite-sync to third
  parties as a hosted or managed service.** The planned notare.dev rendezvous /
  relay Worker is deliberately outside this line — it brokers discovery and
  store-and-forwards opaque ciphertext blobs; it never runs sqlite-sync and never
  sees plaintext (E2E covenant, v0.6 plan §3.5).
- If the product ever grows a hosted sync service, no determination recorded here
  covers it, and it must be re-opened with SQLite Cloud.

**To close this out, in order:**

1. Send the drafted follow-up; archive **both** SQLite Cloud replies (email or
   support-ticket export) alongside this note, so whatever the outcome rests on a
   primary source rather than a summary of one.
2. If a commercial licence is offered, price it against the swap cost before
   assuming either is cheaper.
3. If the denial stands, the vendored tree under `crates/cloudsync/` is removed
   rather than left in place unused.

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
