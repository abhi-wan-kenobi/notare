import { describe, expect, it } from "vitest";

import { splitHighlight } from "./highlight";

describe("splitHighlight", () => {
  it("returns a single non-match part for an empty query", () => {
    expect(splitHighlight("hello world", "")).toEqual([
      { text: "hello world", match: false },
    ]);
  });

  it("returns a single non-match part for empty text", () => {
    expect(splitHighlight("", "hello")).toEqual([{ text: "", match: false }]);
  });

  it("highlights a case-insensitive substring match", () => {
    expect(splitHighlight("The Roadmap review", "roadmap")).toEqual([
      { text: "The ", match: false },
      { text: "Roadmap", match: true },
      { text: " review", match: false },
    ]);
  });

  it("highlights every occurrence of a term", () => {
    expect(splitHighlight("go go go", "go")).toEqual([
      { text: "go", match: true },
      { text: " ", match: false },
      { text: "go", match: true },
      { text: " ", match: false },
      { text: "go", match: true },
    ]);
  });

  it("highlights each whitespace-separated term independently", () => {
    const parts = splitHighlight("action item list", "action item");
    const matched = parts
      .filter((p) => p.match)
      .map((p) => p.text.toLowerCase());
    expect(matched).toContain("action");
    expect(matched).toContain("item");
    // Reassembling the parts must reproduce the original text losslessly.
    expect(parts.map((p) => p.text).join("")).toBe("action item list");
  });

  it("merges adjacent/overlapping matched spans into one part", () => {
    // "meeting" then "meet" overlap; the covered region stays contiguous.
    const parts = splitHighlight("meeting", "meet meeting");
    expect(parts).toEqual([{ text: "meeting", match: true }]);
  });

  it("returns no matches when nothing matches", () => {
    const parts = splitHighlight("hello world", "xyz");
    expect(parts.some((p) => p.match)).toBe(false);
    expect(parts.map((p) => p.text).join("")).toBe("hello world");
  });
});
