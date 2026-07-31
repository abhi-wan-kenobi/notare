/**
 * Custom-dictionary engine (Notare v0.5.1, Lane A3 - the deterministic layer
 * of the dictation cleanup pipeline).
 *
 * The dictionary upgrades the old flat term list (`personalization_dictionary_terms`)
 * into two kinds of entry that share one stored setting:
 *
 * - a bare **string** is a legacy flat term: an STT keyword-bias hint only, it
 *   never rewrites text;
 * - a **mapping** (`{ wrong, right, caseSensitive }`) is a deterministic
 *   wrong -> right replacement applied to dictation text *after* transcription
 *   (its `right` side additionally feeds the STT hints, see `stt/keywords.ts`).
 *
 * Storage stays the existing JSON string array; it now holds strings AND
 * mapping objects. The settings schema treats the value as an opaque string,
 * so nothing there needs to change - but note that `shared/config`'s
 * `parseStringArray` and `chat/tools/session-correction`'s
 * `parseStoredDictionaryTerms` both currently drop the object entries. Reading
 * full entries (this module's `parseDictionaryEntries`) is the way to see them.
 */

export interface DictionaryMapping {
  wrong: string;
  right: string;
  caseSensitive: boolean;
}

/** A bare string is a legacy flat term (STT bias hint only, no replacement). */
export type DictionaryEntry = string | DictionaryMapping;

/** Plain-text import/export separators. */
const ARROW = " => ";
const CS_SUFFIX = " [cs]";

/**
 * A "word char" for boundary detection. Unlike `\b`, this works for
 * non-Latin scripts (Devanagari, CJK, ...) by treating any Unicode letter,
 * number, combining mark or underscore as part of a word. Combining marks
 * (`\p{M}`, e.g. Devanagari matras/viramas) are included so a match can't be
 * declared at the seam inside a grapheme cluster.
 */
const WORD_CHAR = /[\p{L}\p{N}\p{M}_]/u;

function isWordChar(ch: string | undefined): boolean {
  return ch !== undefined && ch.length > 0 && WORD_CHAR.test(ch);
}

/**
 * Word-char test for the full code point ENDING at UTF-16 index `end - 1`.
 * A supplementary-plane letter (e.g. CJK Extension B) is two surrogates;
 * testing the lone low surrogate against WORD_CHAR is false and would open
 * a phantom boundary next to it.
 */
function isWordCharBefore(text: string, index: number): boolean {
  if (index <= 0) {
    return false;
  }
  const unit = text.charCodeAt(index - 1);
  // Low surrogate: the code point starts one unit earlier.
  if (unit >= 0xdc00 && unit <= 0xdfff && index >= 2) {
    const cp = text.codePointAt(index - 2);
    return cp !== undefined && WORD_CHAR.test(String.fromCodePoint(cp));
  }
  return isWordChar(text[index - 1]);
}

/** Word-char test for the full code point STARTING at UTF-16 index `index`. */
function isWordCharAt(text: string, index: number): boolean {
  if (index >= text.length) {
    return false;
  }
  const cp = text.codePointAt(index);
  return cp !== undefined && WORD_CHAR.test(String.fromCodePoint(cp));
}

function isMapping(entry: DictionaryEntry): entry is DictionaryMapping {
  return typeof entry === "object" && entry !== null;
}

/**
 * Parse the stored JSON array into entries. Tolerant of the legacy
 * flat-string-array shape, of mixed arrays, and of garbage (returns `[]`
 * rather than throwing). Malformed mappings (missing/blank `wrong`) are
 * dropped; a missing `right` becomes `""` (a deletion mapping).
 */
export function parseDictionaryEntries(raw: string): DictionaryEntry[] {
  if (typeof raw !== "string" || raw.trim() === "") {
    return [];
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }

  if (!Array.isArray(parsed)) {
    return [];
  }

  const out: DictionaryEntry[] = [];
  for (const item of parsed) {
    if (typeof item === "string") {
      const term = item.trim();
      if (term) {
        out.push(term);
      }
      continue;
    }

    if (item && typeof item === "object") {
      const record = item as Record<string, unknown>;
      const wrong = typeof record.wrong === "string" ? record.wrong : "";
      const right = typeof record.right === "string" ? record.right : "";
      if (wrong.trim() === "") {
        continue;
      }
      out.push({ wrong, right, caseSensitive: Boolean(record.caseSensitive) });
    }
  }

  return out;
}

/** Serialize entries back to the stored JSON string (inverse of parse). */
export function serializeDictionaryEntries(entries: DictionaryEntry[]): string {
  const normalized = entries.map((entry) =>
    isMapping(entry)
      ? {
          wrong: entry.wrong,
          right: entry.right,
          caseSensitive: Boolean(entry.caseSensitive),
        }
      : entry,
  );
  return JSON.stringify(normalized);
}

interface PreparedRule {
  wrong: string;
  wrongLower: string;
  right: string;
  caseSensitive: boolean;
  len: number;
  firstIsWord: boolean;
  lastIsWord: boolean;
}

function prepareRules(entries: DictionaryEntry[]): PreparedRule[] {
  return (
    entries
      .filter(isMapping)
      // Drop empty terms and no-op mappings (`wrong === right`): a no-op can
      // never usefully fire and guarantees no self-replacement loop.
      .filter((m) => m.wrong.length > 0 && m.wrong !== m.right)
      .map((m) => ({
        wrong: m.wrong,
        wrongLower: m.wrong.toLowerCase(),
        right: m.right,
        caseSensitive: m.caseSensitive,
        len: m.wrong.length,
        // Code-point-aware on both edges: an astral-plane letter is two UTF-16
        // units and a lone surrogate never matches WORD_CHAR.
        firstIsWord: isWordCharAt(m.wrong, 0),
        lastIsWord: isWordCharBefore(m.wrong, m.wrong.length),
      }))
      // Longest wrong first: overlapping terms ("notare" vs "note") resolve to
      // the most specific match at any given position.
      .sort((a, b) => b.len - a.len)
  );
}

/**
 * Apply the dictionary's replacement mappings to `text` in a single
 * left-to-right pass. Guarantees:
 *
 * - **word-boundary safe** - a mapping whose `wrong` starts/ends with a word
 *   char won't match mid-word (uses the Unicode-aware `WORD_CHAR` above, so it
 *   holds for Devanagari/CJK where `\b` fails); a `wrong` edged by punctuation
 *   (e.g. `C++`, `**bold**`) relaxes the boundary on that side;
 * - **literal** - `wrong` is matched as a plain string, so regex-special
 *   characters need no escaping;
 * - **longest-wrong-first** precedence for overlaps;
 * - **per-mapping case sensitivity** (case-insensitive by default);
 * - **single pass, no cascading** - an emitted `right` is never re-scanned, so
 *   one rule's output can never be rewritten by another rule in the same call.
 *
 * Flat-string entries are ignored here (they are STT hints, not replacements).
 * Deterministic and synchronous.
 */
export function applyDictionary(
  text: string,
  entries: DictionaryEntry[],
): string {
  if (!text) {
    return text;
  }

  const rules = prepareRules(entries);
  if (rules.length === 0) {
    return text;
  }

  const n = text.length;
  let out = "";
  let i = 0;

  while (i < n) {
    let matched = false;
    const here = text[i];
    const hereLower = here.toLowerCase();

    for (const rule of rules) {
      const j = i + rule.len;
      if (j > n) {
        continue;
      }

      // Cheap first-char reject before slicing.
      if (rule.caseSensitive) {
        if (here !== rule.wrong[0]) {
          continue;
        }
        if (text.slice(i, j) !== rule.wrong) {
          continue;
        }
      } else {
        if (hereLower !== rule.wrongLower[0]) {
          continue;
        }
        if (text.slice(i, j).toLowerCase() !== rule.wrongLower) {
          continue;
        }
      }

      // Boundary check - only enforced on a side whose edge char is a word
      // char, so punctuation-edged terms still match against adjacent letters.
      // Code-point-aware: a surrogate half must not read as a non-word char.
      if (rule.firstIsWord && isWordCharBefore(text, i)) {
        continue;
      }
      if (rule.lastIsWord && isWordCharAt(text, j)) {
        continue;
      }

      out += rule.right;
      i = j;
      matched = true;
      break;
    }

    if (!matched) {
      out += here;
      i += 1;
    }
  }

  return out;
}

/**
 * Import entries from plain text, one per line:
 * - a bare line is a flat term;
 * - `wrong => right` is a mapping (split on the first ` => `, so arrows inside
 *   either side survive);
 * - a trailing ` [cs]` on a mapping marks it case-sensitive.
 * Blank lines are skipped.
 */
export function importDictionaryText(text: string): DictionaryEntry[] {
  const out: DictionaryEntry[] = [];

  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    const arrowIndex = line.indexOf(ARROW);
    if (arrowIndex === -1) {
      out.push(line);
      continue;
    }

    // Both sides are trimmed: hand-typed lines often carry irregular spacing
    // around the arrow ("hello  =>  world"), and a space-edged term in a
    // dictation dictionary is an accident, not intent.
    const wrong = line.slice(0, arrowIndex).trim();
    if (wrong === "") {
      // No usable left-hand side - keep the whole line as a flat term.
      out.push(line);
      continue;
    }

    let right = line.slice(arrowIndex + ARROW.length).trim();
    let caseSensitive = false;
    if (right.endsWith(CS_SUFFIX)) {
      caseSensitive = true;
      right = right.slice(0, right.length - CS_SUFFIX.length).trimEnd();
    }

    out.push({ wrong, right, caseSensitive });
  }

  return out;
}

/** Export entries to the plain-text format (inverse of `importDictionaryText`). */
export function exportDictionaryText(entries: DictionaryEntry[]): string {
  return entries
    .map((entry) =>
      isMapping(entry)
        ? `${entry.wrong}${ARROW}${entry.right}${
            entry.caseSensitive ? CS_SUFFIX : ""
          }`
        : entry,
    )
    .join("\n");
}
