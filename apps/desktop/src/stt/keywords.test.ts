import { describe, expect, it } from "vitest";

import type { DictionaryMapping } from "~/dictation/dictionary";

import {
  formatDictionaryTerms,
  normalizeKeywordList,
  parseDictionaryTermsText,
} from "./keywords";

const map = (
  wrong: string,
  right: string,
  caseSensitive = false,
): DictionaryMapping => ({ wrong, right, caseSensitive });

describe("normalizeKeywordList", () => {
  it("keeps legacy flat strings, trimming and de-duping case-insensitively", () => {
    expect(normalizeKeywordList([" FarEye ", "fareye", "Notare"])).toEqual([
      "FarEye",
      "Notare",
    ]);
  });

  it("drops sub-2-character and empty terms", () => {
    expect(normalizeKeywordList(["a", "", "  ", "ok"])).toEqual(["ok"]);
  });

  it("uses a mapping's right side as the hint (never the wrong side)", () => {
    expect(normalizeKeywordList([map("far eye", "FarEye")])).toEqual(["FarEye"]);
  });

  it("mixes flat terms and mappings, deduping across both", () => {
    expect(
      normalizeKeywordList(["Notare", map("far eye", "FarEye"), "fareye"]),
    ).toEqual(["Notare", "FarEye"]);
  });

  it("ignores a malformed mapping with a non-string right", () => {
    expect(
      normalizeKeywordList([{ wrong: "x", right: 1 } as unknown as DictionaryMapping, "ok"]),
    ).toEqual(["ok"]);
  });
});

describe("parseDictionaryTermsText", () => {
  it("splits on newlines and commas", () => {
    expect(parseDictionaryTermsText("FarEye, Notare\nOllama")).toEqual([
      "FarEye",
      "Notare",
      "Ollama",
    ]);
  });
});

describe("formatDictionaryTerms", () => {
  it("joins normalized hints (including mapping right sides) with newlines", () => {
    expect(formatDictionaryTerms(["FarEye", map("far eye", "Reeye")])).toBe(
      "FarEye\nReeye",
    );
  });
});
