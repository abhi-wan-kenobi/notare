import { describe, expect, test } from "vitest";

import {
  CORE_TRANSCRIPTION_LANGUAGE_CODES,
  getAdditionalSpokenLanguages,
  getBaseLanguageDisplayName,
  HINGLISH_LANGUAGE_CODE,
  parseLocale,
} from "./language";

describe("Hinglish sentinel", () => {
  test("is offered as a transcription option", () => {
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).toContain(HINGLISH_LANGUAGE_CODE);
  });

  test("labels as Hinglish (Intl has no entry for it)", () => {
    expect(getBaseLanguageDisplayName(HINGLISH_LANGUAGE_CODE)).toBe("Hinglish");
  });

  test("survives locale parsing intact instead of collapsing to en", () => {
    expect(parseLocale(HINGLISH_LANGUAGE_CODE).language).toBe(
      HINGLISH_LANGUAGE_CODE,
    );
  });
});

describe("getAdditionalSpokenLanguages", () => {
  test("removes the main language from stored spoken languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["en", "ko"])).toEqual(["ko"]);
  });

  test("matches regional variants by base language", () => {
    expect(getAdditionalSpokenLanguages("en-US", ["en", "ko-KR"])).toEqual([
      "ko",
    ]);
  });

  test("deduplicates additional languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["ko", "ko-KR", "ja"])).toEqual([
      "ko",
      "ja",
    ]);
  });

  test("ignores malformed stored spoken languages", () => {
    expect(getAdditionalSpokenLanguages("en", ["not a locale", "ko"])).toEqual([
      "ko",
    ]);
  });

  test("uses a valid fallback while the main language is loading", () => {
    expect(parseLocale("")).toEqual({ language: "en" });
  });
});

describe("CORE_TRANSCRIPTION_LANGUAGE_CODES", () => {
  test("uses languages supported by both Deepgram and Soniox", () => {
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).toContain("en");
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).toContain("zh");
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).toContain("sr");

    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).not.toContain("af");
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).not.toContain("az");
    expect(CORE_TRANSCRIPTION_LANGUAGE_CODES).not.toContain("sq");
  });
});
