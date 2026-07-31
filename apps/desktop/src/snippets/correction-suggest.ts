/**
 * Word-diff dictionary-correction suggester (Notare v0.5.1, Lane A1).
 *
 * Given a snippet's text before and after a manual edit (Snippets inline
 * edit, or a chat `apply_session_correction` oldText/newText pair), find
 * short contiguous runs of replaced whitespace tokens that look like a term
 * correction ("far eye" -> "FarEye") rather than a prose rewrite. Pure and
 * synchronous - no DB/settings access here, callers own persistence and any
 * "add to dictionary?" confirmation UI (Abhishek's rule: suggest, never
 * silently auto-learn).
 */

export interface CorrectionCandidate {
  wrong: string;
  right: string;
}

/** Cap on candidates returned per (before, after) pair. */
const MAX_CANDIDATES = 3;

/**
 * Input ceiling for the O(n*m) LCS table: beyond this many tokens on either
 * side the edit is prose rewriting, not a term correction, and the quadratic
 * diff would stall the renderer on document-sized text.
 */
const MAX_DIFF_TOKENS = 200;

/**
 * A replaced run longer than this, on either side, is prose editing (a
 * rewritten clause/sentence) rather than a term correction - drop it rather
 * than truncate it, since a partial slice of a rewritten clause is not a
 * usable dictionary mapping.
 */
const MAX_RUN_TOKENS = 3;

type DiffOp =
  | { type: "match"; token: string }
  | { type: "replace"; before: string[]; after: string[] };

function tokenize(text: string): string[] {
  return text.split(/\s+/).filter(Boolean);
}

/**
 * Suffix LCS-length table over token arrays `a` (rows) and `b` (columns),
 * computed backward so `dp[i][j]` = length of the LCS of `a[i:]` and `b[j:]`.
 * Token equality is exact/case-sensitive - that's what makes a case-only
 * change ("far eye" vs "FarEye") show up as a replaced run instead of a
 * silent match, which is exactly the correction this module exists to find.
 */
function lcsSuffixTable(a: string[], b: string[]): number[][] {
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () =>
    new Array<number>(m + 1).fill(0),
  );

  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        a[i] === b[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  return dp;
}

/**
 * Token-level diff of `a` -> `b`: a sequence of matched tokens and
 * contiguous replaced runs (LCS-backed, so it finds the minimal edit rather
 * than a greedy left-to-right guess). A pure insertion or deletion still
 * comes out as a "replace" op with one side empty - callers filter those.
 */
function diffTokens(a: string[], b: string[]): DiffOp[] {
  const dp = lcsSuffixTable(a, b);
  const ops: DiffOp[] = [];
  let pendingBefore: string[] = [];
  let pendingAfter: string[] = [];

  const flushReplace = () => {
    if (pendingBefore.length > 0 || pendingAfter.length > 0) {
      ops.push({ type: "replace", before: pendingBefore, after: pendingAfter });
      pendingBefore = [];
      pendingAfter = [];
    }
  };

  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      flushReplace();
      ops.push({ type: "match", token: a[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      pendingBefore.push(a[i]);
      i++;
    } else {
      pendingAfter.push(b[j]);
      j++;
    }
  }
  while (i < a.length) {
    pendingBefore.push(a[i]);
    i++;
  }
  while (j < b.length) {
    pendingAfter.push(b[j]);
    j++;
  }
  flushReplace();

  return ops;
}

/**
 * Suggest dictionary-mapping candidates from a before/after text pair.
 *
 * A candidate is a contiguous run of 1-3 whitespace tokens on BOTH sides,
 * dropped when: either side is empty (pure insertion/deletion, not a
 * correction), the run exceeds {@link MAX_RUN_TOKENS} tokens on either side
 * (prose editing, not a term correction), or the joined text is identical on
 * both sides (should not occur post-diff, kept as a defensive no-op guard).
 * Case-only changes ARE kept - "far eye" -> "FarEye" is the primary use
 * case. Capped at {@link MAX_CANDIDATES} per call, in order of occurrence.
 */
export function suggestCorrections(
  before: string,
  after: string,
): CorrectionCandidate[] {
  const beforeTokens = tokenize(before);
  const afterTokens = tokenize(after);
  if (beforeTokens.length === 0 || afterTokens.length === 0) {
    return [];
  }
  // The LCS table is O(n*m); unbounded input (a chat correction over a whole
  // document) could stall the renderer - and an edit that large is prose
  // rewriting, not a term correction, anyway.
  if (
    beforeTokens.length > MAX_DIFF_TOKENS ||
    afterTokens.length > MAX_DIFF_TOKENS
  ) {
    return [];
  }

  const ops = diffTokens(beforeTokens, afterTokens);
  const candidates: CorrectionCandidate[] = [];

  for (const op of ops) {
    if (op.type !== "replace") {
      continue;
    }
    if (op.before.length === 0 || op.after.length === 0) {
      continue;
    }
    if (op.before.length > MAX_RUN_TOKENS || op.after.length > MAX_RUN_TOKENS) {
      continue;
    }

    const wrong = op.before.join(" ");
    const right = op.after.join(" ");
    if (wrong === right) {
      continue;
    }

    candidates.push({ wrong, right });
    if (candidates.length >= MAX_CANDIDATES) {
      break;
    }
  }

  return candidates;
}
