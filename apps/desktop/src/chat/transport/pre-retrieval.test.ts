import { describe, expect, it, vi } from "vitest";

import {
  appendCitationGuidance,
  CITATION_SYSTEM_GUIDANCE,
  formatPreRetrievalBlock,
  type RetrievedChunk,
  runPreRetrieval,
} from "./pre-retrieval";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";
import type { LexicalHit, SemanticHit } from "~/search/hybrid";

describe("pre-retrieval", () => {
  describe("formatPreRetrievalBlock", () => {
    it("returns empty string when chunks list is empty", () => {
      expect(formatPreRetrievalBlock([])).toBe("");
    });

    it("formats retrieved chunks with stable citation ids and meeting titles", () => {
      const chunks: RetrievedChunk[] = [
        {
          sessionId: "sess-1",
          startMs: 12000,
          text: "First important discussion point.",
          sourceId: "[[src:sess-1@12000]]",
          meetingTitle: "Sprint Planning",
        },
        {
          sessionId: "sess-2",
          startMs: 0,
          text: "Second important discussion point.",
          sourceId: "[[src:sess-2@0]]",
        },
      ];

      const formatted = formatPreRetrievalBlock(chunks);

      expect(formatted).toContain("<retrieved_meeting_context>");
      expect(formatted).toContain("</retrieved_meeting_context>");
      expect(formatted).toContain(
        '[Source [[src:sess-1@12000]]] (Meeting: "Sprint Planning")',
      );
      expect(formatted).toContain("First important discussion point.");
      expect(formatted).toContain("[Source [[src:sess-2@0]]]");
      expect(formatted).toContain("Second important discussion point.");
    });
  });

  describe("appendCitationGuidance", () => {
    it("appends citation guidance to undefined or empty prompts", () => {
      expect(appendCitationGuidance(undefined)).toBe(CITATION_SYSTEM_GUIDANCE);
      expect(appendCitationGuidance("")).toBe(CITATION_SYSTEM_GUIDANCE);
    });

    it("appends citation guidance to an existing prompt", () => {
      const prompt = "You are a helpful assistant.";
      const result = appendCitationGuidance(prompt);
      expect(result).toBe(`${prompt}\n\n${CITATION_SYSTEM_GUIDANCE}`);
    });
  });

  describe("runPreRetrieval", () => {
    it("returns top-k chunks within scope using hybrid fusion", async () => {
      const lexicalHits: LexicalHit[] = [
        {
          id: "sess-1",
          entityType: "session",
          title: "Meeting 1",
          content: "Lexical content 1",
          score: 1.0,
        },
        {
          id: "sess-2",
          entityType: "session",
          title: "Meeting 2",
          content: "Lexical content 2",
          score: 0.8,
        },
      ];

      const semanticHits: SemanticHit[] = [
        {
          chunkId: "c1",
          sessionId: "sess-1",
          sourceType: "transcript",
          text: "Semantic chunk for sess-1",
          startMs: 5000,
          distance: 0.1,
        },
        {
          chunkId: "c2",
          sessionId: "sess-2",
          sourceType: "transcript",
          text: "Semantic chunk for sess-2",
          startMs: 15000,
          distance: 0.2,
        },
      ];

      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["sess-1"],
      };

      const chunks = await runPreRetrieval({
        query: "important query",
        scopeFilter,
        limit: 8,
        lexicalSearch: vi.fn().mockResolvedValue(lexicalHits),
        semanticSearch: vi.fn().mockResolvedValue(semanticHits),
        resolveAllowedIds: vi.fn().mockResolvedValue(["sess-1"]),
      });

      expect(chunks).toHaveLength(1);
      expect(chunks[0]?.sessionId).toBe("sess-1");
      expect(chunks[0]?.sourceId).toBe("[[src:sess-1@5000]]");
      expect(chunks[0]?.text).toBe("Semantic chunk for sess-1");
      expect(chunks[0]?.meetingTitle).toBe("Meeting 1");
    });

    it("returns empty array when scope restricts to no matching sessions", async () => {
      const chunks = await runPreRetrieval({
        query: "test",
        scopeFilter: { kind: "session_ids", sessionIds: [] },
        resolveAllowedIds: vi.fn().mockResolvedValue([]),
      });

      expect(chunks).toEqual([]);
    });

    it("returns at most the limit (top-k 8 chunks)", async () => {
      const lexicalHits: LexicalHit[] = [
        {
          id: "s1",
          entityType: "session",
          title: "S1",
          content: "Content",
          score: 1,
        },
      ];
      const semanticHits: SemanticHit[] = Array.from(
        { length: 12 },
        (_, i) => ({
          chunkId: `c${i}`,
          sessionId: "s1",
          sourceType: "transcript",
          text: `Chunk ${i}`,
          startMs: i * 1000,
          distance: 0.01 * i,
        }),
      );

      const chunks = await runPreRetrieval({
        query: "multi chunk",
        limit: 8,
        lexicalSearch: vi.fn().mockResolvedValue(lexicalHits),
        semanticSearch: vi.fn().mockResolvedValue(semanticHits),
      });

      expect(chunks).toHaveLength(8);
      expect(chunks[7]?.sourceId).toBe("[[src:s1@7000]]");
    });
  });
});
