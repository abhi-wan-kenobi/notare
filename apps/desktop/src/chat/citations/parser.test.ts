import { describe, expect, it } from "vitest";

import {
  extractCitations,
  formatTimestamp,
  parseCitation,
  parseCitationSegments,
  replaceCitationsForRendering,
} from "./parser";

describe("citation parser", () => {
  describe("parseCitation - ids", () => {
    it("parses valid citation tags", () => {
      expect(parseCitation("[[src:meeting-1@12345]]")).toEqual({
        sessionId: "meeting-1",
        ms: 12345,
      });

      expect(parseCitation("[[src:sess_abc.123:part@0]]")).toEqual({
        sessionId: "sess_abc.123:part",
        ms: 0,
      });
    });

    it("trims whitespace around input", () => {
      expect(parseCitation("  [[src:s1@500]]  ")).toEqual({
        sessionId: "s1",
        ms: 500,
      });
    });
  });

  describe("parseCitation - malformed", () => {
    it("returns null for malformed citations", () => {
      expect(parseCitation("[[src:]]")).toBeNull();
      expect(parseCitation("[[src:s1]]")).toBeNull();
      expect(parseCitation("[[src:@100]]")).toBeNull();
      expect(parseCitation("[[src:s1@abc]]")).toBeNull();
      expect(parseCitation("[[src:s1@-100]]")).toBeNull();
      expect(parseCitation("[[other:s1@100]]")).toBeNull();
      expect(parseCitation("[src:s1@100]")).toBeNull();
      expect(parseCitation("[[src:s1@100]")).toBeNull();
      expect(parseCitation("prefix[[src:s1@100]]")).toBeNull();
      expect(parseCitation("[[src:s1@100]]suffix")).toBeNull();
    });
  });

  describe("extractCitations", () => {
    it("extracts multiple citations from text", () => {
      const text =
        "Found in [[src:sess-1@1000]] and confirmed in [[src:sess-2@54321]].";
      const citations = extractCitations(text);

      expect(citations).toEqual([
        { sessionId: "sess-1", ms: 1000, raw: "[[src:sess-1@1000]]" },
        { sessionId: "sess-2", ms: 54321, raw: "[[src:sess-2@54321]]" },
      ]);
    });

    it("ignores malformed citations while extracting valid ones", () => {
      const text =
        "Broken [[src:broken]] and [[src:@500]] but valid [[src:valid-id@2500]].";
      const citations = extractCitations(text);

      expect(citations).toEqual([
        { sessionId: "valid-id", ms: 2500, raw: "[[src:valid-id@2500]]" },
      ]);
    });
  });

  describe("parseCitationSegments - adjacent text", () => {
    it("handles citations immediately adjacent to text without spaces", () => {
      const text = "Word[[src:s1@1000]]Another[[src:s2@2000]]End";
      const segments = parseCitationSegments(text);

      expect(segments).toEqual([
        { type: "text", text: "Word" },
        { type: "citation", sessionId: "s1", ms: 1000, raw: "[[src:s1@1000]]" },
        { type: "text", text: "Another" },
        { type: "citation", sessionId: "s2", ms: 2000, raw: "[[src:s2@2000]]" },
        { type: "text", text: "End" },
      ]);
    });

    it("handles consecutive adjacent citations with no separating characters", () => {
      const text = "[[src:s1@100]][[src:s2@200]]";
      const segments = parseCitationSegments(text);

      expect(segments).toEqual([
        { type: "citation", sessionId: "s1", ms: 100, raw: "[[src:s1@100]]" },
        { type: "citation", sessionId: "s2", ms: 200, raw: "[[src:s2@200]]" },
      ]);
    });

    it("preserves malformed tags as plain text", () => {
      const text = "Normal text [[src:invalid]] and [[src:@99]] stays text.";
      const segments = parseCitationSegments(text);

      expect(segments).toEqual([
        {
          type: "text",
          text: "Normal text [[src:invalid]] and [[src:@99]] stays text.",
        },
      ]);
    });
  });

  describe("formatTimestamp", () => {
    it("formats milliseconds into MM:SS or HH:MM:SS", () => {
      expect(formatTimestamp(0)).toBe("00:00");
      expect(formatTimestamp(65000)).toBe("01:05");
      expect(formatTimestamp(3723000)).toBe("1:02:03");
    });
  });

  describe("replaceCitationsForRendering", () => {
    it("replaces citations with markdown links", () => {
      const text = "Check [[src:sess-1@1000]] for details.";
      expect(replaceCitationsForRendering(text)).toBe(
        "Check [src:sess-1@1000](citation:sess-1@1000) for details.",
      );
    });

    it("does not rewrite citations inside code blocks", () => {
      const text = "Look at `[[src:s1@1000]]` or:\n```\n[[src:s2@2000]]\n```";
      expect(replaceCitationsForRendering(text)).toBe(text);
    });
  });
});
