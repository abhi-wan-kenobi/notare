import { describe, expect, it } from "vitest";

import { extractActionItems, type GenerateObjectFn } from "./extract";
import type { SpeakerRosterEntry, TranscriptWord } from "./gates";

const TRANSCRIPT =
  "Alice: I'll send the revised budget to finance by Friday. " +
  "Bob: someone should really look into the CRM migration at some point.";

const ROSTER: SpeakerRosterEntry[] = [
  { speakerId: "spk_1", label: "Alice" },
  { speakerId: "spk_2", label: "Bob" },
];
const WORDS: TranscriptWord[] = [
  { text: "I'll", start_ms: 500, end_ms: 600 },
  { text: "send", start_ms: 600, end_ms: 700 },
  { text: "the", start_ms: 700, end_ms: 750 },
  { text: "revised", start_ms: 750, end_ms: 1000 },
  { text: "budget", start_ms: 1000, end_ms: 1300 },
];

const meetingDate = new Date("2026-07-23T10:00:00Z");

/** A stub generateObject that returns queued responses in order. */
function stubGen(responses: unknown[]): {
  fn: GenerateObjectFn;
  calls: number;
} {
  let i = 0;
  const state = { calls: 0 };
  const fn = (async () => {
    state.calls += 1;
    const object = responses[Math.min(i, responses.length - 1)];
    i += 1;
    return { object } as Awaited<ReturnType<GenerateObjectFn>>;
  }) as unknown as GenerateObjectFn;
  return {
    fn,
    get calls() {
      return state.calls;
    },
  } as never;
}

describe("extractActionItems pipeline", () => {
  it("runs extract -> verify -> gates and returns only gated items", async () => {
    const extractResp = {
      action_items: [
        {
          text: "Send revised budget to finance",
          source_text: "I'll send the revised budget", // covered by WORDS
          owner_speaker_id: "spk_1",
          due_at: "Friday",
          confidence: 0.9,
        },
        {
          text: "Migrate the CRM",
          source_text: "we will migrate the CRM next quarter", // fabricated
          owner_speaker_id: "spk_2",
          confidence: 0.4,
        },
      ],
    };
    // Verifier keeps both (simulating an imperfect verifier); the gate must
    // still drop the fabricated one.
    const verifyResp = extractResp;

    const gen = stubGen([extractResp, verifyResp]);
    const result = await extractActionItems(
      {} as never,
      {
        transcript: TRANSCRIPT,
        words: WORDS,
        roster: ROSTER,
        meetingDate,
      },
      { generateObjectFn: (gen as { fn: GenerateObjectFn }).fn },
    );

    expect(result.modelCalls).toBe(2);
    expect(result.kept).toHaveLength(1);
    expect(result.kept[0].source_start_ms).toBe(500);
    expect(result.kept[0].due_at).toBe("2026-07-24");
    expect(result.rejected).toHaveLength(1);
    expect(result.rejected[0].reason).toBe("source_text_not_in_transcript");
  });

  it("skips the verify call when extraction returns nothing", async () => {
    const gen = stubGen([{ action_items: [] }]);
    const result = await extractActionItems(
      {} as never,
      {
        transcript: TRANSCRIPT,
        words: WORDS,
        roster: ROSTER,
        meetingDate,
      },
      { generateObjectFn: (gen as { fn: GenerateObjectFn }).fn },
    );

    expect(result.modelCalls).toBe(1);
    expect(result.kept).toHaveLength(0);
  });

  it("falls back to extraction candidates if verify throws (gates still apply)", async () => {
    const extractResp = {
      action_items: [
        {
          text: "Send revised budget",
          source_text: "send the revised budget to finance",
          owner_speaker_id: "spk_1",
          confidence: 0.7,
        },
      ],
    };
    const throwingGen = (async () => {
      // first call ok, second throws
      if ((throwingGen as { n?: number }).n) throw new Error("verify boom");
      (throwingGen as { n?: number }).n = 1;
      return { object: extractResp } as never;
    }) as unknown as GenerateObjectFn;

    const result = await extractActionItems(
      {} as never,
      {
        transcript: TRANSCRIPT,
        words: WORDS,
        roster: ROSTER,
        meetingDate,
      },
      { generateObjectFn: throwingGen },
    );

    expect(result.kept).toHaveLength(1);
    expect(result.kept[0].owner_speaker_id).toBe("spk_1");
  });
});
