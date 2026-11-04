# Releasing Notare

Stable release checklist. Pre-releases use `-rcN` tags and skip steps 2–3 gates only if they are never promoted; a stable tag always runs the gate.

1. **Bump the version** to the same `X.Y.Z` in both `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.stable.json`. The in-app What's New page reads the first; the release gate and updater read the second.
2. **Write `packages/changelog/content/X.Y.Z.md`**. Front matter needs `date` and a one-sentence plain-text `summary`, then `##` sections of user-facing changes. No PR numbers or internal changes. Format and rules: `packages/changelog/content/AGENTS.md`.
3. **Tag** the merge commit: `git tag vX.Y.Z && git push origin vX.Y.Z`. This triggers `.github/workflows/release.yaml`.
4. **Launch gate.** The `preflight` job fails the run, before any build starts, if `packages/changelog/content/<version in tauri.conf.stable.json>.md` is missing or has no `summary`. Fix the file, delete the tag and re-tag. Then check the release against `docs/release/0.6-production-gate.md` (or the current minor's gate).
5. **Mark latest.** `publish` creates the GitHub release. Once the gate passes, make sure `vX.Y.Z` is flagged *Latest* and not *Pre-release* (`gh release edit vX.Y.Z --latest --prerelease=false`): the updater endpoint follows the latest release.

A version that ships broken is replaced by a new patch, not edited. Give the broken version's changelog file a short "replaced by X.Y.Z" note, as `0.6.2.md` and `0.6.3.md` do.
