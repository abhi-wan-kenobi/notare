import { describe, expect, it } from "vitest";

import { suggestCorrections } from "./correction-suggest";

describe("suggestCorrections", () => {
  it("finds a case-only correction across a multi-word run", () => {
    expect(suggestCorrections("far eye", "FarEye")).toEqual([
      { wrong: "far eye", right: "FarEye" },
    ]);
  });

  it("finds a multi-word term correction amid unchanged context", () => {
    const before = "talked to sam about open world project";
    const after = "talked to sam about OpenWorld project";

    expect(suggestCorrections(before, after)).toEqual([
      { wrong: "open world", right: "OpenWorld" },
    ]);
  });

  it("rejects a run that exceeds 3 tokens on either side (prose editing)", () => {
    const before = "This is a nice day outside";
    const after = "This is a genuinely lovely sunny and warm day outside";

    expect(suggestCorrections(before, after)).toEqual([]);
  });

  it("rejects when the whole text is an unrelated rewrite longer than 3 tokens", () => {
    expect(
      suggestCorrections(
        "the quick brown fox jumps",
        "an entirely different sentence",
      ),
    ).toEqual([]);
  });

  it("caps at 3 candidates even when more short runs are found", () => {
    const before = "alpha one bravo two charlie three delta four";
    const after = "ALPHA one BRAVO two CHARLIE three DELTA four";

    const candidates = suggestCorrections(before, after);
    expect(candidates).toHaveLength(3);
    expect(candidates).toEqual([
      { wrong: "alpha", right: "ALPHA" },
      { wrong: "bravo", right: "BRAVO" },
      { wrong: "charlie", right: "CHARLIE" },
    ]);
  });

  it("returns nothing for an identical no-op edit", () => {
    expect(suggestCorrections("hello world", "hello world")).toEqual([]);
  });

  it("drops a pure insertion (before side empty)", () => {
    expect(suggestCorrections("hello", "hello world")).toEqual([]);
  });

  it("drops a pure deletion (after side empty)", () => {
    expect(suggestCorrections("hello world", "hello")).toEqual([]);
  });

  it("returns nothing when either text is blank", () => {
    expect(suggestCorrections("", "hello")).toEqual([]);
    expect(suggestCorrections("hello", "")).toEqual([]);
    expect(suggestCorrections("   ", "   ")).toEqual([]);
  });

  it("keeps a run at exactly the 3-token cap", () => {
    expect(suggestCorrections("far eye systems", "FarEyeSystems")).toEqual([
      { wrong: "far eye systems", right: "FarEyeSystems" },
    ]);
  });
});
