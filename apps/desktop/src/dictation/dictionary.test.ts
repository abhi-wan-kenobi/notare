import { describe, expect, it } from "vitest";

import {
  applyDictionary,
  type DictionaryEntry,
  exportDictionaryText,
  importDictionaryText,
  parseDictionaryEntries,
  serializeDictionaryEntries,
} from "./dictionary";

const map = (
  wrong: string,
  right: string,
  caseSensitive = false,
): DictionaryEntry => ({ wrong, right, caseSensitive });

describe("parseDictionaryEntries", () => {
  it("parses the legacy flat string array", () => {
    expect(parseDictionaryEntries('["FarEye", "Notare"]')).toEqual([
      "FarEye",
      "Notare",
    ]);
  });

  it("parses mixed strings and mappings", () => {
    expect(
      parseDictionaryEntries(
        '["Notare", {"wrong":"far eye","right":"FarEye","caseSensitive":false}]',
      ),
    ).toEqual(["Notare", map("far eye", "FarEye")]);
  });

  it("coerces a missing caseSensitive to false and a missing right to empty", () => {
    expect(
      parseDictionaryEntries('[{"wrong":"foo"},{"wrong":"a","right":"b","caseSensitive":1}]'),
    ).toEqual([map("foo", ""), map("a", "b", true)]);
  });

  it("drops blank flat terms and mappings with a blank wrong", () => {
    expect(
      parseDictionaryEntries('["  ", {"wrong":"   ","right":"x"}, "keep"]'),
    ).toEqual(["keep"]);
  });

  it("is tolerant of garbage, non-arrays and empty input", () => {
    expect(parseDictionaryEntries("not json")).toEqual([]);
    expect(parseDictionaryEntries('{"wrong":"a"}')).toEqual([]);
    expect(parseDictionaryEntries("")).toEqual([]);
    expect(parseDictionaryEntries("   ")).toEqual([]);
    expect(parseDictionaryEntries("[1, true, null]")).toEqual([]);
  });

  it("round-trips through serialize", () => {
    const entries: DictionaryEntry[] = ["Notare", map("far eye", "FarEye", true)];
    expect(parseDictionaryEntries(serializeDictionaryEntries(entries))).toEqual(
      entries,
    );
  });
});

describe("applyDictionary word boundaries", () => {
  it("replaces a whole-word match", () => {
    expect(applyDictionary("i love far eye", [map("far eye", "FarEye")])).toBe(
      "i love FarEye",
    );
  });

  it("does not replace inside a longer word", () => {
    expect(applyDictionary("noteworthy notes", [map("note", "NOTE")])).toBe(
      "noteworthy notes",
    );
    // ...but a standalone occurrence still fires.
    expect(applyDictionary("a note here", [map("note", "NOTE")])).toBe(
      "a NOTE here",
    );
  });

  it("matches multi-word terms with internal spaces", () => {
    expect(
      applyDictionary("the far eye team", [map("far eye", "FarEye")]),
    ).toBe("the FarEye team");
  });

  it("handles regex-special characters literally", () => {
    expect(applyDictionary("i use c++ daily", [map("c++", "C++")])).toBe(
      "i use C++ daily",
    );
    expect(
      applyDictionary("make it **bold** now", [map("**bold**", "STRONG")]),
    ).toBe("make it STRONG now");
    expect(applyDictionary("a.b.c value", [map("a.b.c", "ABC")])).toBe(
      "ABC value",
    );
  });
});

describe("applyDictionary precedence and passes", () => {
  it("prefers the longest wrong on overlap", () => {
    const entries = [map("note", "NOTE"), map("notare", "Notare")];
    expect(applyDictionary("open notare please", entries)).toBe(
      "open Notare please",
    );
  });

  it("never re-matches an emitted right in the same pass (no cascade)", () => {
    // "a" -> "b" and "b" -> "c": a single pass must yield "b", not "c".
    expect(applyDictionary("a a", [map("a", "b"), map("b", "c")])).toBe("b b");
  });

  it("treats wrong === right as a no-op with no loop", () => {
    expect(applyDictionary("loop loop", [map("loop", "loop")])).toBe(
      "loop loop",
    );
  });

  it("returns text unchanged for an empty dictionary or flat-only entries", () => {
    expect(applyDictionary("nothing here", [])).toBe("nothing here");
    expect(applyDictionary("nothing here", ["FarEye", "Notare"])).toBe(
      "nothing here",
    );
  });
});

describe("applyDictionary case sensitivity", () => {
  it("matches case-insensitively by default", () => {
    expect(applyDictionary("FAR EYE and Far Eye", [map("far eye", "FarEye")])).toBe(
      "FarEye and FarEye",
    );
  });

  it("honors the caseSensitive flag", () => {
    const entries = [map("api", "API", true)];
    expect(applyDictionary("the api and the API", entries)).toBe(
      "the API and the API",
    );
  });

  it("does not fire a case-sensitive rule on a differently-cased token", () => {
    expect(applyDictionary("Api", [map("api", "API", true)])).toBe("Api");
  });
});

describe("applyDictionary Unicode / Devanagari", () => {
  it("replaces a Devanagari term on a boundary", () => {
    // "namaste" written wrong -> corrected, surrounded by spaces.
    expect(applyDictionary("बोलो नमस्ते दोस्त", [map("नमस्ते", "नमस्ते जी")])).toBe(
      "बोलो नमस्ते जी दोस्त",
    );
  });

  it("does not split a Devanagari word mid-cluster", () => {
    // "नमस" must not match inside "नमस्ते" (a combining virama/matra follows).
    expect(applyDictionary("नमस्ते", [map("नमस", "X")])).toBe("नमस्ते");
  });

  it("handles Hinglish (Latin + Devanagari) text", () => {
    expect(
      applyDictionary("mera naam far eye hai", [map("far eye", "FarEye")]),
    ).toBe("mera naam FarEye hai");
  });
});

describe("importDictionaryText / exportDictionaryText", () => {
  it("imports bare lines as flat terms", () => {
    expect(importDictionaryText("FarEye\nNotare")).toEqual(["FarEye", "Notare"]);
  });

  it("imports 'wrong => right' as a mapping", () => {
    expect(importDictionaryText("far eye => FarEye")).toEqual([
      map("far eye", "FarEye"),
    ]);
  });

  it("imports the trailing [cs] marker as caseSensitive", () => {
    expect(importDictionaryText("api => API [cs]")).toEqual([
      map("api", "API", true),
    ]);
  });

  it("skips blank lines", () => {
    expect(importDictionaryText("a => b\n\n   \nc")).toEqual([
      map("a", "b"),
      "c",
    ]);
  });

  it("round-trips flat terms, mappings and [cs]", () => {
    const entries: DictionaryEntry[] = [
      "FarEye",
      map("far eye", "FarEye"),
      map("api", "API", true),
    ];
    expect(importDictionaryText(exportDictionaryText(entries))).toEqual(entries);
  });

  it("round-trips '=>' embedded inside a term (split on the first ' => ')", () => {
    const entries: DictionaryEntry[] = [map("a=>b", "c => d")];
    const text = exportDictionaryText(entries);
    expect(text).toBe("a=>b => c => d");
    expect(importDictionaryText(text)).toEqual(entries);
  });

  it("round-trips '[cs]' appearing inside a term (only a trailing marker counts)", () => {
    const entries: DictionaryEntry[] = [map("x", "y [cs] z")];
    expect(importDictionaryText(exportDictionaryText(entries))).toEqual(entries);
  });
});

describe("applyDictionary performance", () => {
  it("applies 50 rules to a 500-word text well under budget", () => {
    const rules: DictionaryEntry[] = Array.from({ length: 50 }, (_, k) =>
      map(`term${k}`, `TERM${k}`),
    );
    const words = Array.from({ length: 500 }, (_, k) => `term${k % 50}`);
    const text = words.join(" ");

    const started = performance.now();
    const result = applyDictionary(text, rules);
    const elapsed = performance.now() - started;

    expect(result).toContain("TERM0");
    expect(result).not.toContain("term0 ");
    // Generous ceiling for CI noise; the target is < 10ms.
    expect(elapsed).toBeLessThan(50);
  });
});

describe("review-hardening edges", () => {
  it("trims irregular spacing around the arrow on import", () => {
    expect(importDictionaryText("hello  =>  world")).toEqual([
      { wrong: "hello", right: "world", caseSensitive: false },
    ]);
    expect(importDictionaryText("far eye =>  FarEye  [cs]")).toEqual([
      { wrong: "far eye", right: "FarEye", caseSensitive: true },
    ]);
  });

  // Supplementary-plane letters are two UTF-16 units; a boundary test that
  // reads a lone surrogate would wrongly see a non-word char and allow a
  // mid-word match next to them.
  it("treats astral-plane letters as word chars for boundaries", () => {
    const rules = [{ wrong: "note", right: "Notare", caseSensitive: false }];
    // 𠀀 (U+20000, CJK Ext B letter) adjacent on either side: no boundary.
    expect(applyDictionary("𠀀note", rules)).toBe("𠀀note");
    expect(applyDictionary("note𠀀", rules)).toBe("note𠀀");
    // Astral punctuation/emoji stays a non-word char: boundary holds.
    expect(applyDictionary("😀note😀", rules)).toBe("😀Notare😀");
  });
});
