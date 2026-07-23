import { describe, expect, it } from "vitest";

import type { GenerateObjectFn } from "../extract";
import { EVAL_CASES } from "./fixtures";
import { runEval } from "./run";
import { aggregate, formatReport, scoreCase } from "./scoring";

describe("scoring", () => {
  it("computes precision/recall and flags hallucinations", () => {
    const score = scoreCase({
      name: "t",
      transcript: "I'll send the budget to finance by Friday.",
      expected: [
        { source_fragment: "send the budget", owner_speaker_id: "spk_1" },
      ],
      actual: [
        {
          source_text: "I'll send the budget to finance",
          owner_speaker_id: "spk_1",
          due_at: "",
        },
        // a hallucination: not in the transcript
        {
          source_text: "email the board the deck",
          owner_speaker_id: "spk_1",
          due_at: "",
        },
      ],
    });
    expect(score.truePositives).toBe(1);
    expect(score.falsePositives).toBe(1);
    expect(score.hallucinated).toBe(1);
    expect(score.ownerCorrect).toBe(1);
  });

  it("aggregate enforces the release gates", () => {
    const good = aggregate([
      scoreCase({
        name: "a",
        transcript: "book the venue",
        expected: [{ source_fragment: "book the venue", owner_speaker_id: "" }],
        actual: [
          { source_text: "book the venue", owner_speaker_id: "", due_at: "" },
        ],
      }),
    ]);
    expect(good.pass).toBe(true);
    expect(good.precision).toBe(1);

    const halluc = aggregate([
      scoreCase({
        name: "b",
        transcript: "book the venue",
        expected: [],
        actual: [
          {
            source_text: "totally invented task",
            owner_speaker_id: "",
            due_at: "",
          },
        ],
      }),
    ]);
    expect(halluc.pass).toBe(false);
    expect(halluc.failures[0]).toContain("hallucinated");
    expect(formatReport(halluc)).toContain("FAIL");
  });
});

/**
 * A scripted "model" that returns, for each case, its golden expectations
 * verbatim from the transcript PLUS one fabricated item — simulating a decent
 * model that occasionally hallucinates. The structural gate must strip the
 * fabrication so the eval reports hallucination = 0 and precision passes.
 */
function scriptedGen(): GenerateObjectFn {
  return (async (opts: { prompt: string }) => {
    // Identify the case by matching its transcript inside the prompt.
    const c = EVAL_CASES.find((x) => opts.prompt.includes(x.transcript));
    const items = (c?.expected ?? []).map((e) => ({
      text: `do: ${e.source_fragment}`,
      // Reconstruct a verbatim source_text from the transcript around the fragment.
      source_text: verbatimAround(c!.transcript, e.source_fragment),
      owner_speaker_id: e.owner_speaker_id || null,
      // Feed the ISO date straight through; the gate re-parses it.
      due_at: e.due_at ?? null,
      confidence: 0.9,
    }));
    // Always add one fabricated item that is NOT in the transcript.
    items.push({
      text: "fabricated follow-up",
      source_text: "this fabricated sentence is not in any transcript",
      owner_speaker_id: "spk_1",
      due_at: null,
      confidence: 0.95,
    });
    return { object: { action_items: items } } as never;
  }) as unknown as GenerateObjectFn;
}

/** Return a verbatim transcript span that contains `fragment` (case-insensitive). */
function verbatimAround(transcript: string, fragment: string): string {
  const lower = transcript.toLowerCase();
  const i = lower.indexOf(fragment.toLowerCase());
  if (i < 0) return fragment;
  // Extend to a sentence-ish boundary on both sides.
  const start = Math.max(0, transcript.lastIndexOf(":", i) + 1);
  let end = transcript.indexOf(".", i);
  if (end < 0) end = transcript.length;
  return transcript.slice(start, end).trim();
}

describe("runEval (hermetic, scripted model)", () => {
  it("strips the model's fabrication -> hallucination 0, precision passes", async () => {
    const report = await runEval({} as never, {
      generateObjectFn: scriptedGen(),
    });

    // The gate removed every fabricated item.
    expect(report.hallucinated).toBe(0);
    // With fabrications stripped, precision must clear the 0.8 gate.
    expect(report.precision).toBeGreaterThanOrEqual(0.8);
    expect(report.pass).toBe(true);
    // Recall: the scripted model returns every expected item, so recall is high.
    expect(report.recall).toBeGreaterThanOrEqual(0.8);
  });

  it("covers the required fixture categories", () => {
    const names = EVAL_CASES.map((c) => c.name);
    expect(names).toContain("no-items-chitchat");
    expect(names).toContain("someone-should-really-not-a-task");
    expect(names).toContain("implied-owner-from-speaker");
    expect(names).toContain("cross-talk-two-owners");
    expect(names.length).toBeGreaterThanOrEqual(8);
  });
});
