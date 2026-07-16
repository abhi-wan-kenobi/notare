import type { DictationOutputMode } from "@hypr/plugin-dictation";

/**
 * The finish line of a dictation session. The Rust session accumulates the
 * raw transcript and emits `DictationFinishedEvent`; the main-window host
 * hands it to `finalizeDictation`, which:
 *
 * 1. applies the configured cleanup (`dictation_cleanup`):
 *    - "none":  raw text as dictated;
 *    - "basic": the deterministic Rust cleaner (`clean.rs`, via the
 *      `clean_text` command);
 *    - "llm":   the app's configured LLM with a fixed cleanup prompt,
 *      falling back to basic (with a caller-provided notice) when no model
 *      is configured or the call fails;
 * 2. in batch mode, delivers the result: copy to clipboard + paste at the
 *    cursor, or copy-only, per `dictation_paste_at_cursor` (a failed session
 *    degrades to copy-only so nothing is typed into whatever is focused);
 * 3. saves the result to the dictation history. In type mode the segments
 *    were already typed raw while speaking - cleanup only shapes the history
 *    entry.
 *
 * Pure orchestration over injected effects, so the dispatch matrix is unit
 * testable without Tauri.
 */

export const DICTATION_CLEANUP_MODES = ["none", "basic", "llm"] as const;
export type DictationCleanupMode = (typeof DICTATION_CLEANUP_MODES)[number];

export function normalizeCleanupMode(
  raw: string | undefined,
): DictationCleanupMode {
  return raw === "none" || raw === "llm" ? raw : "basic";
}

/** Fixed system prompt for the "llm" cleanup mode. */
export const LLM_CLEANUP_SYSTEM_PROMPT =
  "Clean up dictated text: fix punctuation and casing, remove filler words " +
  "and false starts, preserve the meaning. Return only the cleaned text, " +
  "with no commentary, quotes or markdown.";

export interface FinalizeDictationInput {
  rawText: string;
  mode: DictationOutputMode;
  /** The session died (mic/server failure) instead of stopping cleanly. */
  failed: boolean;
  cleanup: DictationCleanupMode;
  pasteAtCursor: boolean;
}

export interface FinalizeDictationDeps {
  /** Deterministic cleanup (the Rust `clean_text` command). */
  cleanBasic: (text: string) => Promise<string>;
  /**
   * LLM cleanup via the app's configured provider, or `null` when no model
   * is configured (triggers the basic fallback).
   */
  cleanLlm: ((text: string) => Promise<string>) | null;
  /** Batch delivery: copy to clipboard, optionally paste at the cursor. */
  deliver: (text: string, pasteAtCursor: boolean) => Promise<void>;
  saveHistory: (entry: {
    text: string;
    mode: DictationOutputMode;
    cleaned: boolean;
  }) => Promise<void>;
  /** "llm" cleanup fell back to basic (no model / error). */
  onLlmFallback: (error: unknown) => void;
}

export async function finalizeDictation(
  input: FinalizeDictationInput,
  deps: FinalizeDictationDeps,
): Promise<void> {
  const raw = input.rawText.trim();
  if (!raw) {
    return;
  }

  const { text, cleaned } = await cleanTranscript(raw, input.cleanup, deps);
  if (!text) {
    // Cleanup stripped everything (pure non-speech artifacts): nothing worth
    // delivering or remembering.
    return;
  }

  if (input.mode === "batch") {
    try {
      // A failed session degrades to copy-only: the text survives on the
      // clipboard without pasting into whatever happens to be focused.
      await deps.deliver(text, input.pasteAtCursor && !input.failed);
    } catch (error) {
      console.error("[dictation] failed to deliver the transcript", error);
      // Fall through: the history entry below still preserves the text.
    }
  }

  await deps.saveHistory({ text, mode: input.mode, cleaned });
}

async function cleanTranscript(
  raw: string,
  cleanup: DictationCleanupMode,
  deps: FinalizeDictationDeps,
): Promise<{ text: string; cleaned: boolean }> {
  if (cleanup === "none") {
    return { text: raw, cleaned: false };
  }

  if (cleanup === "llm") {
    if (deps.cleanLlm) {
      try {
        const text = (await deps.cleanLlm(raw)).trim();
        if (text) {
          return { text, cleaned: true };
        }
        // An empty LLM answer is a failure, not a cleanup.
        deps.onLlmFallback(new Error("llm returned an empty cleanup"));
      } catch (error) {
        deps.onLlmFallback(error);
      }
    } else {
      deps.onLlmFallback(null);
    }
  }

  return { text: await deps.cleanBasic(raw), cleaned: true };
}
