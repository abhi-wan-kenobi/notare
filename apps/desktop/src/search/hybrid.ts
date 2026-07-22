/**
 * Hybrid search: Reciprocal Rank Fusion (RRF) of the lexical (Tantivy BM25) and
 * dense (sqlite-vec semantic) arms, grouped by meeting/session (WS-B2).
 *
 * RRF is rank-based, so the two arms' incomparable score scales (BM25 vs cosine
 * distance) never have to be normalized: each arm contributes `1 / (k + rank)`
 * to a result's fused score, summed across arms. k=60 is the standard constant.
 *
 * The dense arm is OPTIONAL: with it disabled (S0 NO-GO fallback, or before the
 * index is built) `fuseArms` degenerates to pure Tantivy ranking, so the search
 * page can ship provider-agnostic and light up the semantic arm later.
 */

export const RRF_K = 60;

/** One arm's ranked list: items in descending relevance (rank 0 = best). */
export type RankedItem = { key: string };

export type FusedEntry = {
  key: string;
  /** Summed RRF score across arms. */
  score: number;
  /** 0-based rank in each arm that contributed (for debugging/telemetry). */
  ranks: Record<string, number>;
};

/**
 * Pure RRF over any number of named arms. Each arm is a ranked list; an item's
 * contribution from an arm is `1 / (k + rank)`. Items are keyed so the same
 * result appearing in multiple arms accumulates. Ties broken by key for
 * determinism.
 */
export function fuseArms(
  arms: Record<string, RankedItem[]>,
  k: number = RRF_K,
): FusedEntry[] {
  const acc = new Map<string, FusedEntry>();

  for (const [armName, items] of Object.entries(arms)) {
    items.forEach((item, rank) => {
      let entry = acc.get(item.key);
      if (!entry) {
        entry = { key: item.key, score: 0, ranks: {} };
        acc.set(item.key, entry);
      }
      // Keep the BEST (lowest) rank if a key appears twice within one arm.
      if (!(armName in entry.ranks) || rank < entry.ranks[armName]) {
        // Remove the previous contribution from this arm before re-adding.
        if (armName in entry.ranks) {
          entry.score -= 1 / (k + entry.ranks[armName]);
        }
        entry.ranks[armName] = rank;
        entry.score += 1 / (k + rank);
      }
    });
  }

  return [...acc.values()].sort(
    (a, b) => b.score - a.score || a.key.localeCompare(b.key),
  );
}

// ---- Arm result shapes (mirror the two plugins' bindings) -------------------

export type LexicalHit = {
  /** session/human/organization id (Tantivy document id). */
  id: string;
  entityType: string;
  title: string;
  content: string;
  score: number;
};

export type SemanticHit = {
  chunkId: string;
  sessionId: string;
  sourceType: string;
  text: string;
  startMs: number | null;
  distance: number;
};

export type HybridResult = {
  sessionId: string;
  /** Fused RRF score. */
  score: number;
  /** Best lexical hit for this session, if the lexical arm matched it. */
  lexical?: LexicalHit;
  /** Semantic chunk hits for this session, best-first (for snippet + jump). */
  semantic: SemanticHit[];
};

/**
 * Group both arms by session id and fuse. Only `session`-type lexical hits
 * participate in the session grouping (human/organization hits are lexical-only
 * and returned separately by the caller if desired). Dense arm may be empty.
 */
export function hybridBySession(
  lexical: LexicalHit[],
  semantic: SemanticHit[],
  k: number = RRF_K,
): HybridResult[] {
  const lexicalSessions = lexical.filter((h) => h.entityType === "session");

  const fused = fuseArms(
    {
      lexical: lexicalSessions.map((h) => ({ key: h.id })),
      semantic: semantic.map((h) => ({ key: h.sessionId })),
    },
    k,
  );

  const bestLexical = new Map<string, LexicalHit>();
  for (const h of lexicalSessions) {
    if (!bestLexical.has(h.id)) bestLexical.set(h.id, h); // first = best rank
  }

  const semanticBySession = new Map<string, SemanticHit[]>();
  for (const h of semantic) {
    const list = semanticBySession.get(h.sessionId) ?? [];
    list.push(h);
    semanticBySession.set(h.sessionId, list);
  }
  for (const list of semanticBySession.values()) {
    list.sort((a, b) => a.distance - b.distance);
  }

  return fused.map((entry) => ({
    sessionId: entry.key,
    score: entry.score,
    lexical: bestLexical.get(entry.key),
    semantic: semanticBySession.get(entry.key) ?? [],
  }));
}
