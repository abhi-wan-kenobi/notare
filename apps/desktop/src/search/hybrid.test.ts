import { describe, expect, it } from "vitest";

import {
  fuseArms,
  hybridBySession,
  type LexicalHit,
  RRF_K,
  type SemanticHit,
} from "./hybrid";

describe("fuseArms (RRF core)", () => {
  it("sums 1/(k+rank) across arms and ranks by fused score", () => {
    const fused = fuseArms(
      {
        lexical: [{ key: "a" }, { key: "b" }, { key: "c" }],
        semantic: [{ key: "b" }, { key: "a" }, { key: "d" }],
      },
      RRF_K,
    );
    // lexical [a,b,c] -> a:0 b:1 c:2 ; semantic [b,a,d] -> b:0 a:1 d:2.
    // a = 1/60 + 1/61 ; b = 1/61 + 1/60 (equal to a) -> tie broken by key.
    const score = (key: string) => fused.find((e) => e.key === key)!.score;
    expect(score("a")).toBeCloseTo(1 / 60 + 1 / 61, 10);
    expect(score("b")).toBeCloseTo(1 / 61 + 1 / 60, 10);
    expect(fused[0].key).toBe("a"); // equal score, 'a' < 'b'
    expect(fused[1].key).toBe("b");
    // c and d appear once each at rank 2.
    expect(score("c")).toBeCloseTo(1 / 62, 10);
    expect(score("d")).toBeCloseTo(1 / 62, 10);
  });

  it("degenerates to a single arm's order when only one arm is present", () => {
    const fused = fuseArms({
      lexical: [{ key: "x" }, { key: "y" }, { key: "z" }],
    });
    expect(fused.map((e) => e.key)).toEqual(["x", "y", "z"]);
  });

  it("handles an empty dense arm (NO-GO fallback) as pure lexical", () => {
    const fused = fuseArms({
      lexical: [{ key: "p" }, { key: "q" }],
      semantic: [],
    });
    expect(fused.map((e) => e.key)).toEqual(["p", "q"]);
  });

  it("keeps the best rank when a key repeats within an arm", () => {
    const fused = fuseArms({
      semantic: [{ key: "a" }, { key: "b" }, { key: "a" }],
    });
    // 'a' should score from its rank-0 appearance only, not rank-2.
    expect(fused.find((e) => e.key === "a")!.score).toBeCloseTo(1 / 60, 10);
    expect(fused[0].key).toBe("a");
  });

  it("is deterministic on ties (breaks by key)", () => {
    const fused = fuseArms({
      lexical: [{ key: "z" }],
      semantic: [{ key: "a" }],
    });
    expect(fused.map((e) => e.key)).toEqual(["a", "z"]); // equal score → key order
  });
});

describe("hybridBySession", () => {
  const lexical: LexicalHit[] = [
    {
      id: "s1",
      entityType: "session",
      title: "Budget review",
      content: "...",
      score: 9,
    },
    {
      id: "s2",
      entityType: "session",
      title: "Offsite",
      content: "...",
      score: 5,
    },
    { id: "h9", entityType: "human", title: "Alice", content: "...", score: 8 },
  ];
  const semantic: SemanticHit[] = [
    {
      chunkId: "c1",
      sessionId: "s2",
      sourceType: "transcript",
      text: "book the venue",
      startMs: 1000,
      distance: 0.1,
    },
    {
      chunkId: "c2",
      sessionId: "s1",
      sourceType: "note",
      text: "revised budget",
      startMs: null,
      distance: 0.2,
    },
    {
      chunkId: "c3",
      sessionId: "s2",
      sourceType: "transcript",
      text: "offsite plan",
      startMs: 4000,
      distance: 0.3,
    },
  ];

  it("fuses by session and only sessions participate (humans excluded)", () => {
    const results = hybridBySession(lexical, semantic);
    const ids = results.map((r) => r.sessionId);
    expect(ids).toContain("s1");
    expect(ids).toContain("s2");
    expect(ids).not.toContain("h9"); // human lexical hit not grouped as a session
  });

  it("attaches best lexical hit + distance-sorted semantic chunks per session", () => {
    const results = hybridBySession(lexical, semantic);
    const s2 = results.find((r) => r.sessionId === "s2")!;
    expect(s2.lexical?.title).toBe("Offsite");
    expect(s2.semantic.map((h) => h.chunkId)).toEqual(["c1", "c3"]); // 0.1 before 0.3
    expect(s2.semantic[0].startMs).toBe(1000); // jump target available
  });

  it("returns semantic-only sessions when lexical missed them", () => {
    const results = hybridBySession(
      [],
      [
        {
          chunkId: "c",
          sessionId: "sX",
          sourceType: "note",
          text: "t",
          startMs: null,
          distance: 0.05,
        },
      ],
    );
    expect(results).toHaveLength(1);
    expect(results[0].sessionId).toBe("sX");
    expect(results[0].lexical).toBeUndefined();
  });

  it("works with the dense arm disabled (lexical-only)", () => {
    const results = hybridBySession(lexical, []);
    expect(results.map((r) => r.sessionId).sort()).toEqual(["s1", "s2"]);
    expect(results.every((r) => r.semantic.length === 0)).toBe(true);
  });
});
