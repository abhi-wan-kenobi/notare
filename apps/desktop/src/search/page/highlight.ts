/**
 * Split `text` into alternating non-match / match parts for a case-insensitive
 * substring search of `query`. Whitespace-separated query terms are each matched
 * independently, so "action item" highlights both "action" and "item".
 *
 * Pure + synchronous so it can be unit-tested without the DOM; the React layer
 * wraps every `{ match: true }` part in a `<mark>`.
 */
export type HighlightPart = { text: string; match: boolean };

export function splitHighlight(text: string, query: string): HighlightPart[] {
  const terms = Array.from(
    new Set(
      query
        .toLowerCase()
        .split(/\s+/)
        .map((t) => t.trim())
        .filter((t) => t.length > 0),
    ),
  );

  if (terms.length === 0 || text.length === 0) {
    return [{ text, match: false }];
  }

  const lower = text.toLowerCase();

  // Mark every character covered by any term match.
  const covered = new Array<boolean>(text.length).fill(false);
  for (const term of terms) {
    let from = 0;
    while (from <= lower.length - term.length) {
      const idx = lower.indexOf(term, from);
      if (idx === -1) break;
      for (let i = idx; i < idx + term.length; i++) covered[i] = true;
      from = idx + term.length;
    }
  }

  const parts: HighlightPart[] = [];
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i === text.length || covered[i] !== covered[start]) {
      parts.push({ text: text.slice(start, i), match: covered[start] });
      start = i;
    }
  }
  return parts;
}
