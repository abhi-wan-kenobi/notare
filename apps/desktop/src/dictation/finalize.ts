import type { DictationOutputMode } from "@hypr/plugin-dictation";

import { applyDictionary, type DictionaryEntry } from "./dictionary";
import type { DictationHistorySource, DictationHistoryStatus } from "./history";

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

/**
 * Translation-mode system prompt. Plain, XML-free: translate the dictated
 * speech into `target`, still stripping fillers + fixing punctuation, but
 * never adding/answering/summarizing/continuing - only the translated text
 * comes back. `target == source` degenerates into normal cleanup, which is
 * exactly what we want (a legitimate no-op).
 */
export function buildTranslationSystemPrompt(target: string): string {
  const language = translationLanguageName(target);
  return (
    `Translate the dictated speech into ${language}. ` +
    "Remove filler words and false starts, and fix punctuation and casing. " +
    "Do not add, answer, summarize, explain, or continue the text - only " +
    `translate what was said into ${language}. Return only the translated ` +
    "text, with no commentary, quotes or markdown."
  );
}

/**
 * Turn the stored target ("en", "hi", or a typed language name) into a name
 * the prompt can use - "translate into en" reads poorly to small models. The
 * setting is a raw string interpolated into the system prompt, so anything
 * that doesn't look like a language code/name (prompt-injection shaped, over-
 * long, punctuation-heavy) falls back to English rather than being trusted.
 */
function translationLanguageName(target: string): string {
  const trimmed = target.trim();
  if (!trimmed) {
    return "English";
  }
  if (/^[a-z]{2,3}(-[A-Za-z0-9]{2,8})?$/i.test(trimmed)) {
    try {
      const name = new Intl.DisplayNames(["en"], { type: "language" }).of(
        trimmed,
      );
      if (name && name.toLowerCase() !== trimmed.toLowerCase()) {
        return name;
      }
    } catch {
      // Unknown code: fall through to the shape check below.
    }
  }
  return /^\p{L}[\p{L}\s()-]{0,39}$/u.test(trimmed) ? trimmed : "English";
}

/**
 * Hallucination guard: the LLM cleanup output must not balloon past this
 * multiple of the input's WORD count (measured in words, not characters, so
 * long-word languages aren't unfairly clipped). A model that runs off and
 * "continues" the dictation - the classic small-model failure - blows past
 * this and is discarded in favour of the deterministic rule cleanup.
 */
export const LLM_CLEANUP_MAX_GROWTH_RATIO = 1.3;

/**
 * Translation legitimately changes length (word counts differ across
 * languages), so its guard is deliberately looser than cleanup's while still
 * catching a model that runs away.
 */
export const LLM_TRANSLATION_MAX_GROWTH_RATIO = 3;

/**
 * Inputs longer than this many words are split into chunks (on sentence
 * boundaries, falling back to word slices) and cleaned per chunk, so a single
 * huge prompt can't blow the model's context or its latency budget.
 */
export const LLM_CHUNK_MAX_WORDS = 500;

/**
 * Hard ceiling on the WHOLE LLM pass (all chunks). The paste must never wait
 * on a wedged model, so when this elapses the pass is abandoned and the
 * deterministic rule-cleaned text is delivered instead.
 */
export const LLM_PASS_TIMEOUT_MS = 15_000;

export interface FinalizeDictationInput {
  rawText: string;
  mode: DictationOutputMode;
  /** The session died (mic/server failure) instead of stopping cleanly. */
  failed: boolean;
  cleanup: DictationCleanupMode;
  /**
   * Custom-dictionary entries (`personalization_dictionary_terms`). The
   * mapping entries are applied as deterministic wrong -> right replacements
   * after the basic/LLM cleanup and before delivery + history. Flat-string
   * entries are ignored here (they are STT bias hints, not replacements).
   * Optional so callers that predate the dictionary keep compiling.
   */
  dictionary?: DictionaryEntry[];
  pasteAtCursor: boolean;
  /**
   * Translation mode (`dictation_translation_enabled` +
   * `dictation_translation_target`). When enabled, the LLM pass translates the
   * dictated speech into `target` instead of only cleaning it, using a looser
   * length guard (translation legitimately changes length). Requires
   * `deps.cleanLlm`; when the model is unreachable or the guard trips, the
   * fallback is the rule-cleaned SOURCE text (never blocks, never errors the
   * paste). Optional so callers that predate translation keep compiling.
   */
  translation?: { enabled: boolean; target: string };
  /** STT model that produced the transcript, when the host knows it. */
  model?: string | null;
  /** Wall-clock session length in ms, when the host tracked it. */
  durationMs?: number | null;
}

export interface FinalizeDictationDeps {
  /** Deterministic cleanup (the Rust `clean_text` command). */
  cleanBasic: (text: string) => Promise<string>;
  /**
   * LLM cleanup via the app's configured provider, or `null` when no model
   * is configured (triggers the rule-cleaned fallback). The system prompt is
   * chosen by finalize (cleanup vs translation) and passed in per call so the
   * host stays a thin `generateText` wrapper.
   */
  cleanLlm:
    | ((
        text: string,
        systemPrompt: string,
        signal?: AbortSignal,
      ) => Promise<string>)
    | null;
  /** Batch delivery: copy to clipboard, optionally paste at the cursor. */
  deliver: (text: string, pasteAtCursor: boolean) => Promise<void>;
  saveHistory: (entry: {
    text: string;
    rawText: string | null;
    mode: DictationOutputMode;
    cleaned: boolean;
    source: DictationHistorySource;
    model: string | null;
    durationMs: number | null;
    status: DictationHistoryStatus;
  }) => Promise<void>;
  /**
   * The LLM pass (cleanup or translation) fell back wholesale to the
   * deterministic rule-cleaned text. `error` distinguishes the cause so the
   * host can log it precisely: `null` = no model configured; an `Error` whose
   * message names the trip (empty answer, hallucination guard, timeout) or the
   * original thrown error otherwise. A partial fallback (some chunks kept
   * their LLM output) does NOT fire this - only a total fallback does.
   */
  onLlmFallback: (error: unknown) => void;
  /**
   * Batch-mode finalize can be slow (LLM cleanup) and the Rust session has
   * already gone idle by the time it runs, so the orb would sit idle while
   * the paste is still on its way. When provided, this is called with
   * "processing" before cleanup starts and "idle" once delivery finished
   * (or failed), so the orb reflects the whole cleanup-then-paste window.
   * Only invoked for clean (non-failed) batch sessions - a failed session
   * keeps the error state the Rust side emitted.
   */
  signalPhase?: (phase: "processing" | "idle") => void;
}

export async function finalizeDictation(
  input: FinalizeDictationInput,
  deps: FinalizeDictationDeps,
): Promise<void> {
  const raw = input.rawText.trim();
  if (!raw) {
    return;
  }

  // Keep the orb in "processing" across cleanup + delivery: the paste can
  // trail the session end by seconds on the LLM path and must not land
  // while the orb already claims to be idle.
  const signalPhase =
    input.mode === "batch" && !input.failed ? deps.signalPhase : undefined;
  signalPhase?.("processing");

  try {
    // The cleanup pipeline must never cost the user their dictation: any
    // failure inside it (Rust clean command, dictionary, an LLM-path bug that
    // slips the per-chunk handling) degrades to delivering the raw transcript
    // - and the history write below still happens.
    let text: string;
    let cleaned: boolean;
    try {
      ({ text, cleaned } = await cleanTranscript(raw, input, deps));
    } catch (error) {
      console.error(
        "[dictation] cleanup failed - delivering the raw transcript",
        error,
      );
      text = raw;
      cleaned = false;
    }

    // Discarded-dictation recovery: a session that died, or whose cleanup
    // stripped everything down to non-speech artifacts, delivered nothing
    // usable - but the raw transcript is still worth keeping so it can be
    // recovered from history. `raw` is guaranteed non-empty here, so we never
    // persist a blank entry.
    const status: DictationHistoryStatus =
      input.failed || !text ? "discarded" : "delivered";

    if (text && input.mode === "batch") {
      try {
        // A failed session degrades to copy-only: the text survives on the
        // clipboard without pasting into whatever happens to be focused.
        await deps.deliver(text, input.pasteAtCursor && !input.failed);
      } catch (error) {
        console.error("[dictation] failed to deliver the transcript", error);
        // Fall through: the history entry below still preserves the text.
      }
    }

    await deps.saveHistory({
      text,
      rawText: raw,
      mode: input.mode,
      cleaned,
      source: "dictation",
      model: input.model ?? null,
      durationMs: input.durationMs ?? null,
      status,
    });
  } finally {
    signalPhase?.("idle");
  }
}

async function cleanTranscript(
  raw: string,
  input: FinalizeDictationInput,
  deps: FinalizeDictationDeps,
): Promise<{ text: string; cleaned: boolean }> {
  const { cleanup } = input;
  const dictionary = input.dictionary ?? [];
  const translationEnabled = input.translation?.enabled === true;

  // Deterministic rule-cleaned text. This is BOTH the LLM's input and its
  // fallback: the dictionary runs here, BEFORE the LLM pass, so the model (or
  // translator) sees STT misrecognitions already corrected, and it is never
  // re-applied afterwards. "none" cleanup skips the rule cleaner but still
  // takes the dictionary (its only deterministic rewrite). The raw transcript
  // itself is never touched.
  const ruleBase = cleanup === "none" ? raw : await deps.cleanBasic(raw);
  const ruleText = applyDictionary(ruleBase, dictionary);

  // The entry is "cleaned" (no longer verbatim-raw) if any cleanup mode ran or
  // the dictionary rewrote something.
  const ruleCleaned = cleanup !== "none" || ruleText !== raw;

  // The LLM pass runs for the "llm" cleanup mode OR whenever translation is
  // on (translation replaces the cleanup prompt). Everything else is
  // deterministic and returns here.
  const wantLlm = translationEnabled || cleanup === "llm";
  if (!wantLlm) {
    return { text: ruleText, cleaned: ruleCleaned };
  }

  if (!deps.cleanLlm) {
    deps.onLlmFallback(null);
    return { text: ruleText, cleaned: ruleCleaned };
  }

  const systemPrompt = translationEnabled
    ? buildTranslationSystemPrompt(input.translation?.target ?? "")
    : LLM_CLEANUP_SYSTEM_PROMPT;
  const maxGrowthRatio = translationEnabled
    ? LLM_TRANSLATION_MAX_GROWTH_RATIO
    : LLM_CLEANUP_MAX_GROWTH_RATIO;

  const pass = await runLlmPass(
    ruleText,
    deps.cleanLlm,
    systemPrompt,
    maxGrowthRatio,
  );

  if (!pass.deliveredLlm) {
    // Total fallback (no chunk kept its LLM output, or the pass timed out):
    // deliver the rule-cleaned SOURCE text and tell the host why.
    deps.onLlmFallback(pass.error ?? null);
    return { text: ruleText, cleaned: ruleCleaned };
  }

  return { text: pass.text, cleaned: true };
}

/** Word count used by the hallucination guard + chunker (whitespace-split). */
function countWords(text: string): number {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
}

/**
 * The hallucination guard: an LLM chunk output is acceptable only when it is
 * non-empty and no longer than `maxGrowthRatio` times the input word count.
 * An exactly-at-the-boundary output passes; anything beyond is discarded.
 */
function passesGuard(
  input: string,
  output: string,
  maxGrowthRatio: number,
): boolean {
  if (!output.trim()) {
    return false;
  }
  const inputWords = countWords(input);
  if (inputWords === 0) {
    // A wordless input (punctuation-only chunk) gives the ratio nothing to
    // anchor on - any model output would pass unchecked, so keep the rule
    // text instead.
    return false;
  }
  return countWords(output) <= inputWords * maxGrowthRatio;
}

/**
 * Split a transcript into chunks of at most `LLM_CHUNK_MAX_WORDS` words,
 * preferring sentence boundaries. A single over-long sentence is sliced by
 * word count so no chunk ever exceeds the ceiling. Short transcripts return a
 * single chunk (the common case = one LLM call).
 */
export function chunkTranscript(
  text: string,
  maxWords = LLM_CHUNK_MAX_WORDS,
): string[] {
  if (countWords(text) <= maxWords) {
    return [text];
  }

  // Keep terminal punctuation attached to its sentence.
  const sentences = text.match(/[^.!?]+[.!?]*\s*/g) ?? [text];
  const chunks: string[] = [];
  let current = "";
  let currentWords = 0;

  const flush = () => {
    const trimmed = current.trim();
    if (trimmed) {
      chunks.push(trimmed);
    }
    current = "";
    currentWords = 0;
  };

  for (const sentence of sentences) {
    const words = countWords(sentence);
    if (words > maxWords) {
      // A single sentence larger than the ceiling: slice it by word count.
      flush();
      const tokens = sentence.trim().split(/\s+/);
      for (let i = 0; i < tokens.length; i += maxWords) {
        chunks.push(tokens.slice(i, i + maxWords).join(" "));
      }
      continue;
    }
    if (currentWords + words > maxWords) {
      flush();
    }
    current += sentence;
    currentWords += words;
  }
  flush();

  return chunks.length > 0 ? chunks : [text];
}

interface LlmPassResult {
  text: string;
  /** At least one chunk kept its LLM output (not a total fallback). */
  deliveredLlm: boolean;
  /** Cause of a total fallback, for `onLlmFallback`. */
  error?: unknown;
}

/**
 * Run the LLM pass over `ruleText`: chunk it, clean each chunk sequentially,
 * apply the guard per chunk (a failed chunk falls back to that chunk's
 * rule-cleaned text), and re-join. The whole pass is bounded by
 * `LLM_PASS_TIMEOUT_MS` - if it elapses, the deterministic `ruleText` is
 * returned so delivery is never blocked on a wedged model.
 */
async function runLlmPass(
  ruleText: string,
  cleanLlm: (
    text: string,
    systemPrompt: string,
    signal?: AbortSignal,
  ) => Promise<string>,
  systemPrompt: string,
  maxGrowthRatio: number,
): Promise<LlmPassResult> {
  const chunks = chunkTranscript(ruleText, LLM_CHUNK_MAX_WORDS);
  // The timeout aborts the in-flight provider call AND stops the chunk loop:
  // without this, a wedged first chunk would quietly keep burning one
  // provider call per remaining chunk after the user already got the
  // fallback paste.
  const abort = new AbortController();

  const work = (async (): Promise<LlmPassResult> => {
    const out: string[] = [];
    let deliveredLlm = false;
    let error: unknown;

    for (const chunk of chunks) {
      if (abort.signal.aborted) {
        out.push(chunk.trim());
        continue;
      }
      try {
        const answer = (
          await cleanLlm(chunk, systemPrompt, abort.signal)
        ).trim();
        if (passesGuard(chunk, answer, maxGrowthRatio)) {
          out.push(answer);
          deliveredLlm = true;
        } else {
          // Trimmed: sentence-split chunks keep their trailing whitespace,
          // which would double up around the single-space rejoin below.
          out.push(chunk.trim());
          error = answer
            ? new Error(
                "llm output tripped the hallucination guard " +
                  `(> ${maxGrowthRatio}x input length)`,
              )
            : new Error("llm returned an empty answer");
        }
      } catch (chunkError) {
        out.push(chunk.trim());
        error = chunkError;
      }
    }

    return { text: out.join(" "), deliveredLlm, error };
  })();

  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<LlmPassResult>((resolve) => {
    timer = setTimeout(() => {
      abort.abort();
      resolve({
        text: ruleText,
        deliveredLlm: false,
        error: new Error(
          `llm pass exceeded the ${LLM_PASS_TIMEOUT_MS}ms budget`,
        ),
      });
    }, LLM_PASS_TIMEOUT_MS);
  });

  try {
    return await Promise.race([work, timeout]);
  } finally {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
  }
}
