import { describe, expect, it } from "vitest";

import {
  applyGates,
  locateSourceStartMs,
  normalizeForMatch,
  resolveDueDate,
  resolveOwner,
  sourceTextIsInTranscript,
  type RawActionItem,
  type SpeakerRosterEntry,
  type TranscriptWord,
} from "./gates";

const TRANSCRIPT =
  "Alice: I'll send the revised budget to finance by Friday. " +
  "Bob: Great, and can someone book the venue for the offsite? " +
  "Alice: Sure, I'll handle the venue booking next week.";

const NORM = normalizeForMatch(TRANSCRIPT);

const ROSTER: SpeakerRosterEntry[] = [
  { speakerId: "spk_1", label: "Alice" },
  { speakerId: "spk_2", label: "Bob" },
];

const WORDS: TranscriptWord[] = [
  { text: "I'll", start_ms: 1000, end_ms: 1100, speaker: "spk_1" },
  { text: "send", start_ms: 1100, end_ms: 1200, speaker: "spk_1" },
  { text: "the", start_ms: 1200, end_ms: 1250, speaker: "spk_1" },
  { text: "revised", start_ms: 1250, end_ms: 1500, speaker: "spk_1" },
  { text: "budget", start_ms: 1500, end_ms: 1800, speaker: "spk_1" },
  { text: "to", start_ms: 1800, end_ms: 1850, speaker: "spk_1" },
  { text: "finance", start_ms: 1850, end_ms: 2200, speaker: "spk_1" },
];

describe("normalizeForMatch", () => {
  it("lowercases, strips punctuation, collapses whitespace", () => {
    expect(normalizeForMatch("  I'll   send THE budget! ")).toBe(
      "i ll send the budget",
    );
  });
});

describe("gate 1: verbatim source substring", () => {
  it("accepts an exact quote (modulo punctuation/case/whitespace)", () => {
    expect(
      sourceTextIsInTranscript("I'll send the revised budget to finance", NORM),
    ).toBe(true);
    expect(
      sourceTextIsInTranscript("book the venue for the offsite", NORM),
    ).toBe(true);
  });

  it("REJECTS a paraphrase / fabricated quote (anti-hallucination)", () => {
    expect(
      sourceTextIsInTranscript("Alice will email the marketing deck", NORM),
    ).toBe(false);
    expect(
      sourceTextIsInTranscript("send the budget to accounting", NORM),
    ).toBe(false);
  });

  it("rejects empty source text", () => {
    expect(sourceTextIsInTranscript("", NORM)).toBe(false);
    expect(sourceTextIsInTranscript("   ", NORM)).toBe(false);
  });
});

describe("gate 2: owner in roster or null", () => {
  it("keeps a roster owner", () => {
    expect(resolveOwner("spk_1", ROSTER)).toBe("spk_1");
  });
  it("clears an off-roster (invented) owner to null", () => {
    expect(resolveOwner("spk_99", ROSTER)).toBe("");
    expect(resolveOwner("Alice", ROSTER)).toBe("");
  });
  it("treats null/undefined as null owner", () => {
    expect(resolveOwner(null, ROSTER)).toBe("");
    expect(resolveOwner(undefined, ROSTER)).toBe("");
  });
});

describe("gate 3: due date resolution", () => {
  const meeting = new Date("2026-07-23T10:00:00Z"); // a Thursday

  it("passes through ISO dates", () => {
    expect(resolveDueDate("2026-08-01", meeting)).toBe("2026-08-01");
    expect(resolveDueDate("2026-08-01T09:00:00Z", meeting)).toBe("2026-08-01");
  });
  it("resolves relative forms against the meeting date", () => {
    expect(resolveDueDate("today", meeting)).toBe("2026-07-23");
    expect(resolveDueDate("tomorrow", meeting)).toBe("2026-07-24");
    expect(resolveDueDate("in 3 days", meeting)).toBe("2026-07-26");
    expect(resolveDueDate("next week", meeting)).toBe("2026-07-30");
  });
  it("resolves weekday names to the NEXT such weekday", () => {
    // Thursday 2026-07-23 -> next Friday is 2026-07-24; next Monday 2026-07-27.
    expect(resolveDueDate("Friday", meeting)).toBe("2026-07-24");
    expect(resolveDueDate("next Monday", meeting)).toBe("2026-07-27");
    // "Thursday" on a Thursday means the following Thursday, not today.
    expect(resolveDueDate("Thursday", meeting)).toBe("2026-07-30");
  });
  it("returns '' for vague / past / nonsense inputs (no guessing)", () => {
    expect(resolveDueDate("soon", meeting)).toBe("");
    expect(resolveDueDate("asap", meeting)).toBe("");
    expect(resolveDueDate("yesterday", meeting)).toBe("");
    expect(resolveDueDate("", meeting)).toBe("");
    expect(resolveDueDate(null, meeting)).toBe("");
    expect(resolveDueDate("whenever we get to it", meeting)).toBe("");
  });
});

describe("gate 4: source_start_ms location", () => {
  it("finds the start_ms of the matching word span", () => {
    expect(locateSourceStartMs("the revised budget", WORDS)).toBe(1200);
    expect(locateSourceStartMs("I'll send", WORDS)).toBe(1000);
  });
  it("returns null when the text is not alignable / no words", () => {
    expect(locateSourceStartMs("book the venue", WORDS)).toBeNull();
    expect(locateSourceStartMs("anything", [])).toBeNull();
  });
});

describe("applyGates (integration)", () => {
  const meeting = new Date("2026-07-23T10:00:00Z");

  it("keeps valid items, drops fabricated ones, and enforces every gate", () => {
    const raw: RawActionItem[] = [
      {
        text: "Send the revised budget to finance",
        // Verbatim quote that the word-timing list (up to "finance") covers.
        source_text: "I'll send the revised budget to finance",
        owner_speaker_id: "spk_1",
        due_at: "Friday",
        priority: "high",
        confidence: 0.9,
      },
      {
        // fabricated quote -> must be rejected
        text: "Email the marketing deck",
        source_text: "Alice will email the marketing deck to the board",
        owner_speaker_id: "spk_1",
        due_at: "tomorrow",
        confidence: 0.8,
      },
      {
        // real task, but owner invented + vague date -> kept, owner nulled, date ''
        text: "Book the offsite venue",
        source_text: "book the venue for the offsite",
        owner_speaker_id: "spk_77",
        due_at: "soon",
        priority: "bogus",
        confidence: 2,
      },
      { text: "", source_text: "whatever", confidence: 0.5 }, // empty text
    ];

    const { kept, rejected } = applyGates(raw, {
      transcript: TRANSCRIPT,
      words: WORDS,
      roster: ROSTER,
      meetingDate: meeting,
    });

    expect(kept).toHaveLength(2);
    expect(rejected).toHaveLength(2);
    expect(rejected.map((r) => r.reason).sort()).toEqual([
      "empty_text",
      "source_text_not_in_transcript",
    ]);

    const [budget, venue] = kept;
    expect(budget.owner_speaker_id).toBe("spk_1");
    expect(budget.due_at).toBe("2026-07-24");
    expect(budget.source_start_ms).toBe(1000);
    expect(budget.priority).toBe("high");
    expect(budget.confidence).toBe(0.9);

    // invented owner cleared, vague date cleared, bad priority cleared,
    // confidence clamped, and no word-timing match -> null start.
    expect(venue.owner_speaker_id).toBe("");
    expect(venue.due_at).toBe("");
    expect(venue.priority).toBe("");
    expect(venue.confidence).toBe(1);
    expect(venue.source_start_ms).toBeNull();
  });

  it("RELEASE GATE: zero fabricated source_text survives", () => {
    const fabricated: RawActionItem[] = Array.from({ length: 20 }, (_, i) => ({
      text: `made up task ${i}`,
      source_text: `this sentence ${i} never appears in the meeting`,
      owner_speaker_id: "spk_1",
      confidence: 0.99,
    }));
    const { kept } = applyGates(fabricated, {
      transcript: TRANSCRIPT,
      words: WORDS,
      roster: ROSTER,
      meetingDate: meeting,
    });
    expect(kept).toHaveLength(0);
  });
});
