/**
 * Golden-eval runner for action-item extraction (WS-C PR17).
 *
 * Runs the real `extractActionItems` pipeline over EVAL_CASES with a supplied
 * model (or an injected `generateObjectFn` for a hermetic/scripted run) and
 * reports precision / recall / owner+due accuracy / hallucination vs the golden
 * expectations. The structural substring gate guarantees hallucination = 0; the
 * eval CONFIRMS it and measures the quality the gate can't (precision/recall).
 *
 * Live usage (against coruscant ollama, outside CI) is documented in
 * eval/README — construct an AI-SDK ollama model and pass it as `model`.
 */

import type { LanguageModel } from "ai";

import { extractActionItems, type GenerateObjectFn } from "../extract";
import { EVAL_CASES } from "./fixtures";
import { aggregate, type EvalReport, scoreCase } from "./scoring";

export async function runEval(
  model: LanguageModel,
  deps: { generateObjectFn?: GenerateObjectFn } = {},
): Promise<EvalReport> {
  const cases = [];
  for (const c of EVAL_CASES) {
    const result = await extractActionItems(
      model,
      {
        transcript: c.transcript,
        words: c.words,
        roster: c.roster,
        meetingDate: new Date(`${c.meetingDate}T10:00:00Z`),
      },
      deps,
    );
    cases.push(
      scoreCase({
        name: c.name,
        transcript: c.transcript,
        expected: c.expected,
        actual: result.kept.map((k) => ({
          source_text: k.source_text,
          owner_speaker_id: k.owner_speaker_id,
          due_at: k.due_at,
        })),
      }),
    );
  }
  return aggregate(cases);
}
