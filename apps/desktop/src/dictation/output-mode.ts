import type { DictationOutputMode } from "@hypr/plugin-dictation";

/**
 * The `dictation_output_mode` setting is a plain string in the settings
 * schema; these helpers are the single place that maps whatever was persisted
 * onto the plugin's `DictationOutputMode` union.
 *
 * History: before the paste-at-cursor rework the setting had a third-ish
 * value `"batch-paste"` (batch mode with the paste baked in). It now splits
 * into `"batch"` + the separate `dictation_paste_at_cursor` boolean, so the
 * legacy value normalizes to `"batch"` (with paste-at-cursor defaulting to
 * true, which is exactly what `"batch-paste"` did).
 */
export const LEGACY_BATCH_PASTE_MODE = "batch-paste";

export function normalizeOutputMode(
  raw: string | undefined,
): DictationOutputMode {
  return raw === "batch" || raw === LEGACY_BATCH_PASTE_MODE ? "batch" : "type";
}

/** Whether the persisted value still uses the pre-rework spelling. */
export function isLegacyOutputMode(raw: string | undefined): boolean {
  return raw === LEGACY_BATCH_PASTE_MODE;
}
