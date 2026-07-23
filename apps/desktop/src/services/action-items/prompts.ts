/**
 * Prompt construction for action-item extraction (WS-C).
 *
 * The prompts push the model toward the behavior the code gates then ENFORCE:
 * verbatim source quotes, owners drawn only from the closed roster (or null),
 * and no invented dates. The gates in gates.ts are the guarantee; the prompt
 * only improves the hit rate.
 */

import type { SpeakerRosterEntry } from "./gates";

export type ExtractPromptInput = {
  transcript: string;
  roster: SpeakerRosterEntry[];
  /** Absolute meeting date, injected so the model resolves relative dates. */
  meetingDateIso: string;
};

function rosterBlock(roster: SpeakerRosterEntry[]): string {
  if (roster.length === 0) {
    return "(no identified speakers — every owner MUST be null)";
  }
  return roster
    .map((r) => `- id="${r.speakerId}"${r.label ? ` (${r.label})` : ""}`)
    .join("\n");
}

export function buildExtractPrompt(input: ExtractPromptInput): string {
  return [
    "You extract concrete action items from a meeting transcript.",
    "",
    `The meeting took place on ${input.meetingDateIso}. Resolve any relative`,
    'due dates ("tomorrow", "next Monday", "in 3 days") against THAT date.',
    "",
    "Rules you MUST follow:",
    "1. Only extract items that represent a real, actionable commitment or task.",
    "   Do NOT invent tasks. If there are none, return an empty list.",
    "2. `source_text` MUST be copied VERBATIM from the transcript — an exact,",
    "   contiguous quote of the sentence(s) the task comes from. Do not",
    "   paraphrase, summarize, or fix grammar in source_text. If you cannot",
    "   quote it verbatim, do not emit the item.",
    "3. `owner_speaker_id` MUST be one of the speaker ids listed below, or null.",
    "   NEVER guess an owner. If it is not explicitly clear who owns the task",
    "   from the transcript, set owner_speaker_id to null.",
    "4. `due_at`: an ISO date (YYYY-MM-DD) if a concrete deadline is stated or",
    "   clearly implied, else null. Never invent a deadline.",
    "5. `text` is your own concise imperative phrasing of the task.",
    "6. `confidence` in [0,1]: how sure you are this is a real, correctly-",
    "   attributed action item.",
    "",
    "Speaker roster (the ONLY valid owner ids):",
    rosterBlock(input.roster),
    "",
    "Transcript:",
    '"""',
    input.transcript,
    '"""',
  ].join("\n");
}

export function buildVerifyPrompt(input: {
  transcript: string;
  roster: SpeakerRosterEntry[];
  candidatesJson: string;
}): string {
  return [
    "You are a strict verifier for extracted action items. For EACH candidate,",
    "decide whether to KEEP or DROP it, and correct it if needed.",
    "",
    "Drop a candidate when ANY of these hold:",
    "- source_text is not an exact, contiguous quote from the transcript.",
    "- the item is not a real actionable task (it's chit-chat, a question, or",
    '  a vague aspiration like "we should think about X someday").',
    "- owner_speaker_id is not clearly supported by the transcript (set it to",
    "  null instead of dropping, if the task itself is real).",
    "",
    "Return the KEPT candidates only, corrected. Keep source_text verbatim.",
    "Valid owner ids: " +
      (input.roster.length
        ? input.roster.map((r) => `"${r.speakerId}"`).join(", ")
        : "(none — owners must be null)") +
      ".",
    "",
    "Transcript:",
    '"""',
    input.transcript,
    '"""',
    "",
    "Candidates:",
    input.candidatesJson,
  ].join("\n");
}
