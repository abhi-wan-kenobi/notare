/**
 * Build the `extractActionItems` input (transcript / words / roster) and a
 * speaker -> display-label map from a loaded `SessionContentSnapshot`.
 *
 * The roster is the CLOSED set of diarization speaker ids seen in the words —
 * gate 2 in the extraction pipeline only keeps an owner_speaker_id that is a
 * member of this set, so it must be derived from the same words the model sees.
 *
 * Labels are best-effort: a speaker id that resolves to a session participant
 * (its human id) shows that participant's name; otherwise it gets a stable
 * "Speaker N" label. When neither is resolvable the owner chip is simply hidden
 * downstream (documented acceptable edge case for this PR).
 */

import type { ExtractInput } from "~/services/action-items/extract";
import type { SessionContentSnapshot } from "~/session/content-queries";
import type { WordWithId } from "~/stt/types";

type SpeakerRosterEntry = ExtractInput["roster"][number];
type TranscriptWord = ExtractInput["words"][number];

export type ExtractionInput = {
  transcript: string;
  words: TranscriptWord[];
  roster: SpeakerRosterEntry[];
  /** speakerId -> human-readable label, for rendering the owner chip. */
  labelBySpeakerId: Map<string, string>;
};

function flattenWords(snapshot: SessionContentSnapshot): WordWithId[] {
  return snapshot.transcripts.flatMap((transcript) => transcript.words);
}

export function buildExtractionInput(
  snapshot: SessionContentSnapshot,
): ExtractionInput {
  const words = flattenWords(snapshot);

  // Map any speaker id that matches a participant human id to that name.
  const participantNameByHumanId = new Map(
    snapshot.participants
      .filter((participant) => participant.humanId && participant.name)
      .map((participant) => [participant.humanId, participant.name] as const),
  );

  const labelBySpeakerId = new Map<string, string>();
  let unknownSpeakerCount = 0;
  const labelFor = (speakerId: string): string => {
    const existing = labelBySpeakerId.get(speakerId);
    if (existing) {
      return existing;
    }
    const participantName = participantNameByHumanId.get(speakerId);
    const label = participantName ?? `Speaker ${++unknownSpeakerCount}`;
    labelBySpeakerId.set(speakerId, label);
    return label;
  };

  // Group contiguous same-speaker runs into labelled turns so the model can
  // attribute owners, while keeping the word text verbatim so gate 1's
  // source_text substring check still matches.
  const lines: string[] = [];
  let currentSpeaker: string | undefined;
  let buffer: string[] = [];
  const flush = () => {
    if (buffer.length === 0) {
      return;
    }
    const prefix = currentSpeaker ? `${labelFor(currentSpeaker)}: ` : "";
    lines.push(`${prefix}${buffer.join(" ")}`);
    buffer = [];
  };

  for (const word of words) {
    const speaker = word.speaker || undefined;
    if (speaker !== currentSpeaker) {
      flush();
      currentSpeaker = speaker;
    }
    buffer.push(word.text ?? "");
  }
  flush();

  const roster: SpeakerRosterEntry[] = [...labelBySpeakerId.entries()].map(
    ([speakerId, label]) => ({ speakerId, label }),
  );

  return {
    transcript: lines.join("\n"),
    words: words.map((word) => ({
      text: word.text ?? "",
      start_ms: word.start_ms ?? 0,
      end_ms: word.end_ms ?? 0,
      speaker: word.speaker,
    })),
    roster,
    labelBySpeakerId,
  };
}
