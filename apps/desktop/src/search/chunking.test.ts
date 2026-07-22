import { describe, expect, it } from "vitest";

import {
  chunkNoteText,
  chunkSession,
  chunkTranscriptWords,
  contentHash,
  type ChunkWord,
} from "./chunking";

describe("contentHash", () => {
  it("is stable and content-sensitive", () => {
    expect(contentHash("hello world")).toBe(contentHash("hello world"));
    expect(contentHash("hello world")).not.toBe(contentHash("hello worlds"));
    expect(contentHash("a")).toMatch(/^[0-9a-f]{8}$/);
  });
});

describe("chunkNoteText", () => {
  it("returns nothing for empty text", () => {
    expect(chunkNoteText("")).toEqual([]);
    expect(chunkNoteText("   \n  ")).toEqual([]);
  });

  it("keeps a short note as a single note chunk with null start_ms", () => {
    const chunks = chunkNoteText("Discuss the Q3 roadmap. Assign owners.");
    expect(chunks).toHaveLength(1);
    expect(chunks[0].source_type).toBe("note");
    expect(chunks[0].start_ms).toBeNull();
    expect(chunks[0].text).toContain("Q3 roadmap");
    expect(chunks[0].content_hash).toMatch(/^[0-9a-f]{8}$/);
  });

  it("splits long text into multiple ~800-char chunks on sentence boundaries", () => {
    const sentence =
      "This is a reasonably long sentence about project planning and delivery. ";
    const long = sentence.repeat(40); // ~2900 chars
    const chunks = chunkNoteText(long);
    expect(chunks.length).toBeGreaterThan(1);
    // No chunk wildly exceeds the target (allow one sentence of overshoot).
    for (const c of chunks) {
      expect(c.text.length).toBeLessThan(800 + sentence.length);
    }
  });

  it("folds a short trailing sliver into the previous chunk", () => {
    const big =
      "A fairly long sentence that carries some real weight here. ".repeat(15);
    const chunks = chunkNoteText(`${big} Tiny tail.`);
    // The "Tiny tail." must not be its own chunk.
    expect(chunks.some((c) => c.text.trim() === "Tiny tail.")).toBe(false);
    expect(chunks[chunks.length - 1].text).toContain("Tiny tail.");
  });
});

describe("chunkTranscriptWords", () => {
  const words: ChunkWord[] = [
    { text: "Let's", start_ms: 1000 },
    { text: "book", start_ms: 1200 },
    { text: "the", start_ms: 1300 },
    { text: "venue", start_ms: 1400 },
  ];

  it("carries the first word's start_ms and marks source transcript", () => {
    const chunks = chunkTranscriptWords(words);
    expect(chunks).toHaveLength(1);
    expect(chunks[0].source_type).toBe("transcript");
    expect(chunks[0].start_ms).toBe(1000);
    expect(chunks[0].text).toBe("Let's book the venue");
  });

  it("emits multiple chunks for a long transcript, each with its own start_ms", () => {
    const many: ChunkWord[] = Array.from({ length: 400 }, (_, i) => ({
      text: "word",
      start_ms: i * 100,
    }));
    const chunks = chunkTranscriptWords(many);
    expect(chunks.length).toBeGreaterThan(1);
    expect(chunks[0].start_ms).toBe(0);
    expect(chunks[1].start_ms).toBeGreaterThan(0);
    expect(chunks[1].start_ms).toBe(chunks[1].start_ms); // defined
  });

  it("ignores empty words and returns nothing for no words", () => {
    expect(chunkTranscriptWords([])).toEqual([]);
    expect(chunkTranscriptWords([{ text: "  ", start_ms: 1 }])).toEqual([]);
  });
});

describe("chunkSession", () => {
  it("combines note + enhanced + transcript and dedups by content hash", () => {
    const chunks = chunkSession({
      rawMarkdown: "Kickoff notes about the migration.",
      enhancedMarkdown: ["Kickoff notes about the migration."], // identical -> deduped within source
      transcriptWords: [
        { text: "we", start_ms: 500 },
        { text: "migrate", start_ms: 700 },
      ],
    });
    const notes = chunks.filter((c) => c.source_type === "note");
    const transcripts = chunks.filter((c) => c.source_type === "transcript");
    expect(notes).toHaveLength(1); // duplicate enhanced note text deduped
    expect(transcripts).toHaveLength(1);
    expect(transcripts[0].start_ms).toBe(500);
  });

  it("handles a session with only a transcript", () => {
    const chunks = chunkSession({
      transcriptWords: [{ text: "hello", start_ms: 0 }],
    });
    expect(chunks).toHaveLength(1);
    expect(chunks[0].source_type).toBe("transcript");
  });
});
