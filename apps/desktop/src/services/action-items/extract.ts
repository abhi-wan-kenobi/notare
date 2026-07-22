/**
 * Action-item extraction pipeline (WS-C).
 *
 * Flow: resolveModel('action_items') -> extract (generateObject) ->
 * verify (generateObject) -> structural gates (gates.ts) -> GatedActionItem[].
 *
 * The two model calls are best-effort quality boosters; correctness is owned by
 * `applyGates`, which is pure and independently tested. The router guarantees
 * the model meets the action_items capability floor (structured outputs + >=7B)
 * or resolution fails up-front — so we never run this against a model that will
 * silently ignore the JSON schema.
 */

import { generateObject, type LanguageModel } from "ai";
import { z } from "zod";

import {
  applyGates,
  type GatedActionItem,
  type GateResult,
  type SpeakerRosterEntry,
  type TranscriptWord,
} from "./gates";
import { buildExtractPrompt, buildVerifyPrompt } from "./prompts";

const rawItemSchema = z.object({
  text: z.string(),
  source_text: z.string(),
  owner_speaker_id: z.string().nullable().optional(),
  due_at: z.string().nullable().optional(),
  priority: z.enum(["low", "medium", "high"]).nullable().optional(),
  confidence: z.number().min(0).max(1).nullable().optional(),
});

const extractionSchema = z.object({
  action_items: z.array(rawItemSchema),
});

export type ExtractInput = {
  transcript: string;
  words: TranscriptWord[];
  roster: SpeakerRosterEntry[];
  meetingDate: Date;
};

export type ExtractOutput = GateResult & {
  /** Model calls that ran (for latency/telemetry surfaces). */
  modelCalls: number;
};

/**
 * `generateObjectFn` is injected so tests drive the pipeline deterministically
 * without a live model. Production passes the AI-SDK `generateObject`.
 */
export type GenerateObjectFn = typeof generateObject;

export async function extractActionItems(
  model: LanguageModel,
  input: ExtractInput,
  deps: { generateObjectFn?: GenerateObjectFn } = {},
): Promise<ExtractOutput> {
  const gen = deps.generateObjectFn ?? generateObject;
  const meetingDateIso = input.meetingDate.toISOString().slice(0, 10);

  // Call 1 — extract.
  const extracted = await gen({
    model,
    schema: extractionSchema,
    prompt: buildExtractPrompt({
      transcript: input.transcript,
      roster: input.roster,
      meetingDateIso,
    }),
  });
  const candidates = extracted.object.action_items ?? [];

  let verified = candidates;
  let modelCalls = 1;

  // Call 2 — verify (only when there's something to check).
  if (candidates.length > 0) {
    try {
      const verifyResult = await gen({
        model,
        schema: extractionSchema,
        prompt: buildVerifyPrompt({
          transcript: input.transcript,
          roster: input.roster,
          candidatesJson: JSON.stringify(candidates, null, 2),
        }),
      });
      verified = verifyResult.object.action_items ?? [];
      modelCalls = 2;
    } catch {
      // If verification fails, fall back to the extraction candidates — the
      // structural gates below are the real guarantee either way.
      verified = candidates;
    }
  }

  const gated = applyGates(verified, {
    transcript: input.transcript,
    words: input.words,
    roster: input.roster,
    meetingDate: input.meetingDate,
  });

  return { ...gated, modelCalls };
}

export type { GatedActionItem };
