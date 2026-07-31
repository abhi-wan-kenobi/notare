import type { DictionaryMapping } from "~/dictation/dictionary";

/**
 * An STT keyword-bias hint source. Dictionary entries are now either legacy
 * flat terms (bare strings) or wrong -> right mappings; only the flat term
 * itself, or a mapping's `right` (the corrected spelling we want the model to
 * prefer), is useful as a bias hint. The `wrong` side is deliberately excluded.
 */
type KeywordSource = string | DictionaryMapping;

function keywordHint(source: KeywordSource): string | null {
  if (typeof source === "string") {
    return source;
  }
  return typeof source?.right === "string" ? source.right : null;
}

/**
 * Normalize and de-duplicate a keyword list. Accepts legacy flat strings and
 * dictionary mappings interchangeably (a mapping contributes its `right`),
 * so callers that already hold a plain `string[]` are unaffected.
 */
export function normalizeKeywordList(words: readonly KeywordSource[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const word of words) {
    const hint = keywordHint(word);
    if (hint === null) {
      continue;
    }
    const normalized = hint.trim().replace(/\s+/g, " ");
    const key = normalized.toLocaleLowerCase();
    if (normalized.length < 2 || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(normalized);
  }

  return result;
}

export function parseDictionaryTermsText(value: string): string[] {
  return normalizeKeywordList(
    value
      .split(/[\n,]/)
      .map((term) => term.trim())
      .filter(Boolean),
  );
}

export function formatDictionaryTerms(terms: readonly KeywordSource[]): string {
  return normalizeKeywordList(terms).join("\n");
}
