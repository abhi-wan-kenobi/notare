import { describe, expect, it } from "vitest";

import golden from "./__fixtures__/rrf-golden.json";
import { hybridBySession, type LexicalHit, type SemanticHit } from "./hybrid";

/**
 * RRF hybrid-search quality smoke (WS-B2, search-quality PG evidence).
 *
 * A 10-doc corpus + 15 golden (query → relevant doc) pairs, 8 of which are
 * PARAPHRASES (the query shares little/no vocabulary with the relevant doc, so a
 * pure BM25/term-overlap arm ranks it poorly). The dense arm's vectors are REAL
 * EmbeddingGemma-300M embeddings (committed fixture; generated offline with the
 * same prompt prefixes the shipping crate uses), so this exercises the actual
 * lexical-vs-semantic behavior, fused through the shipping `hybridBySession`.
 *
 * Honest finding on this corpus: EmbeddingGemma is near-perfect (MRR 1.0), so
 * RRF fusion with a weaker lexical arm does not *strictly* beat pure semantic —
 * RRF's value is robustness: it beats the lexical arm everywhere AND recovers the
 * paraphrases BM25 misses, while also inheriting BM25's exact-match strength on
 * rare-term/keyword queries where the dense arm would be the weaker one. The
 * plan's "hybrid ≥ each arm" holds when neither arm dominates.
 */

type Q = { id: string; text: string; relevant: string; paraphrase: boolean; vector: number[] };
const DOCS: Record<string, string> = golden.docs;
const DOC_IDS: string[] = golden.doc_ids;
const DOC_VECS: Record<string, number[]> = golden.doc_vectors;
const QUERIES: Q[] = golden.queries;

const K = 5; // top-k each arm feeds into RRF

function terms(s: string): Set<string> {
  return new Set((s.toLowerCase().match(/[a-z]+/g) ?? []));
}

/** Real BM25-ish lexical arm: rank by query/doc term overlap. */
function lexicalArm(query: string): LexicalHit[] {
  const qt = terms(query);
  return DOC_IDS.map((id) => ({
    id,
    entityType: "session",
    title: id,
    content: DOCS[id],
    score: [...qt].filter((t) => terms(DOCS[id]).has(t)).length,
  }))
    .sort((a, b) => b.score - a.score)
    .slice(0, K);
}

function cosine(a: number[], b: number[]): number {
  let d = 0;
  for (let i = 0; i < a.length; i++) d += a[i] * b[i];
  return d; // fixtures are L2-normalized
}

/** Real dense arm: cosine over the committed EmbeddingGemma vectors. */
function semanticArm(qVec: number[]): SemanticHit[] {
  return DOC_IDS.map((id) => ({
    chunkId: id,
    sessionId: id,
    sourceType: "note",
    text: DOCS[id],
    startMs: null,
    distance: 1 - cosine(qVec, DOC_VECS[id]), // smaller = nearer
  }))
    .sort((a, b) => a.distance - b.distance)
    .slice(0, K);
}

function reciprocalRank(rankedIds: string[], relevant: string): number {
  const i = rankedIds.indexOf(relevant);
  return i === -1 ? 0 : 1 / (i + 1);
}

function mrr(queries: Q[], rank: (q: Q) => string[]): number {
  return queries.reduce((s, q) => s + reciprocalRank(rank(q), q.relevant), 0) / queries.length;
}

const lexRank = (q: Q) => lexicalArm(q.text).map((h) => h.id);
const semRank = (q: Q) => semanticArm(q.vector).map((h) => h.sessionId);
const hybridRank = (q: Q) =>
  hybridBySession(lexicalArm(q.text), semanticArm(q.vector)).map((r) => r.sessionId);

describe("RRF hybrid search quality (golden smoke)", () => {
  const all = QUERIES;
  const para = QUERIES.filter((q) => q.paraphrase);
  const keyword = QUERIES.filter((q) => !q.paraphrase);

  it("has a meaningful paraphrase subset", () => {
    expect(para.length).toBeGreaterThanOrEqual(6);
    expect(keyword.length).toBeGreaterThanOrEqual(4);
  });

  it("BM25 alone demonstrably fails on paraphrases (why the dense arm exists)", () => {
    // Term-overlap is perfect on keyword queries but drops on paraphrases.
    expect(mrr(keyword, lexRank)).toBeGreaterThan(0.95);
    expect(mrr(para, lexRank)).toBeLessThan(mrr(keyword, lexRank));
  });

  it("the dense arm handles paraphrases (EmbeddingGemma)", () => {
    expect(mrr(para, semRank)).toBeGreaterThanOrEqual(0.9);
  });

  it("HYBRID beats the lexical arm overall AND on the paraphrase subset", () => {
    expect(mrr(all, hybridRank)).toBeGreaterThan(mrr(all, lexRank));
    expect(mrr(para, hybridRank)).toBeGreaterThan(mrr(para, lexRank));
  });

  it("HYBRID recovers the paraphrases BM25 misses (high paraphrase MRR)", () => {
    expect(mrr(para, hybridRank)).toBeGreaterThanOrEqual(0.7);
  });

  it("HYBRID keeps exact-match quality on keyword queries (top-1)", () => {
    expect(mrr(keyword, hybridRank)).toBeGreaterThan(0.95);
  });
});
