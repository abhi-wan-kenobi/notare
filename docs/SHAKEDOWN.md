# Release shakedown checklist

Manual pre-release checks for behaviour that **can't be unit-tested** — native
OS integration (macOS AppKit, Windows windowing), synthetic input, GPU offload,
and end-to-end flows. Unit/integration tests (`pnpm -F desktop test`,
`cargo test`) gate correctness in CI; this list covers the rest. Run the
relevant sections on each platform before tagging a release.

History: every item here exists because a real shakedown caught a bug the test
suite structurally could not (e.g. the v0.3.1 paste-at-cursor crash).

## Core flow (all platforms)
- [ ] Fresh install → onboarding → pick a vault folder.
- [ ] Download an STT model; record a short meeting → transcript appears.
- [ ] Enhance/notes → note saved as Markdown into the vault folder.
- [ ] Auto-update: an older build sees the new `latest.json`, downloads, restarts into the new version.

## Dictation
- [ ] Global hotkey starts/stops dictation; text lands in the focused app.
- [ ] **Paste-at-cursor mode does NOT crash** (regression: v0.3.1 SIGTRAP via enigo TSM on a worker thread — macOS). Verify on macOS specifically.
- [ ] Type-at-cursor mode works.
- [ ] **Processing indicator** is visible on the orb after you stop speaking (transcribing state).
- [ ] Orb is a reasonable size / visible.

## macOS-specific (AppKit — compile-checked only, never unit-tested)
- [ ] **Click the dictation orb** toggles dictation (not just the hotkey).
- [ ] **Meeting-bar buttons** (stop / captions / open) respond on first click.
- [ ] Orb + meeting bar stay visible when **switching Spaces**. (Full-screen apps hiding the orb = known limitation.)
- [ ] Signed builds only: no "damaged"/Gatekeeper dialog; **Accessibility permission persists** across relaunches (broken on ad-hoc builds — needs Developer-ID signing).

## Windows-specific
- [ ] Orb appears and is usable across multiple monitors.
- [ ] Switching orb design in Settings does **not** shift the settings page.
- [ ] WASAPI loopback capture works (record other participants).

## STT companion server (if used)
- [ ] Custom STT server URL (`https://…/v1`) + token connects; connection test shows engine + model.
- [ ] Transcription routes to the server; **GPU offload actually happens** (admin page "GPU verified Nx", or watch GPU utilisation).
- [ ] Token gate: `/v1/listen` 401s without the token, works with it.

## Parakeet (ONNX) GPU offload
- [ ] **macOS: confirm Parakeet loads with the CoreML EP** and falls back to CPU if unavailable (watch the `parakeet_execution_provider_active` / `..._falling_back_to_cpu` log lines). Real CoreML acceleration can only be verified on Abhishek's Mac — Linux CI only typechecks the path.

## On-device speaker diarization (#15 — needs a real multi-speaker recording)
Unit-testable pieces are covered (alignment 14 tests, model-load, command compiles);
these end-to-end paths need a live recording on real hardware:
- [ ] Record/import a **2-3 speaker** meeting with a **local** engine (Whisper `Quantized*` or Parakeet). After transcription finishes, speaker labels (**"Speaker 1/2/3"**) appear on the transcript turns (may pop in a few seconds after the text, as the diarization post-pass completes).
- [ ] The split is roughly correct (turns attributed to the right speaker); tune with the "# of speakers" hint once P2.6 lands.
- [ ] Diarization does **not** run for cloud providers that already diarize (no double-labeling), and a diarization failure never fails the transcription (transcript still saves).
- [ ] Runs **before** audio-retention deletion (labels still appear even with retention on).

## Per-fix rule
When a bug fix touches a path with no feasible unit test (native input, GPU,
OS windowing), add a line here in the same PR.
