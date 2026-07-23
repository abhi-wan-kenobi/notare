/**
 * Golden fixture set for the action-item eval (WS-C PR17).
 *
 * Each case is a small meeting transcript + the action items a correct
 * extractor should produce. Coverage mirrors the plan's required categories:
 * no-items, implied owner, relative dates, "someone should really…" (a
 * non-commitment that must NOT become an item), cross-talk, and clean cases.
 */

import type { SpeakerRosterEntry, TranscriptWord } from "../gates";
import type { ExpectedItem } from "./scoring";

export type EvalCase = {
  name: string;
  transcript: string;
  words: TranscriptWord[];
  roster: SpeakerRosterEntry[];
  /** ISO meeting date for relative-date resolution. */
  meetingDate: string;
  expected: ExpectedItem[];
};

const R2: SpeakerRosterEntry[] = [
  { speakerId: "spk_1", label: "Alice" },
  { speakerId: "spk_2", label: "Bob" },
];

/** Build trivial word timings from a transcript (each word 300ms apart). */
function words(text: string, startMs = 0): TranscriptWord[] {
  return text
    .split(/\s+/)
    .filter(Boolean)
    .map((w, i) => ({
      text: w,
      start_ms: startMs + i * 300,
      end_ms: startMs + i * 300 + 250,
    }));
}

export const EVAL_CASES: EvalCase[] = [
  {
    name: "clean-single-owner-relative-date",
    transcript: "Alice: I'll send the revised budget to finance by Friday.",
    words: words("I'll send the revised budget to finance by Friday", 1000),
    roster: R2,
    meetingDate: "2026-07-23", // Thursday -> Friday = 2026-07-24
    expected: [
      {
        source_fragment: "send the revised budget to finance",
        owner_speaker_id: "spk_1",
        due_at: "2026-07-24",
      },
    ],
  },
  {
    name: "no-items-chitchat",
    transcript:
      "Alice: How was your weekend? Bob: Pretty good, went hiking. Alice: Nice, the weather was great.",
    words: words(
      "How was your weekend Pretty good went hiking Nice the weather was great",
    ),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [],
  },
  {
    name: "someone-should-really-not-a-task",
    // A vague aspiration with no owner/commitment — must NOT become an item.
    transcript:
      "Bob: Someone should really look into the CRM migration at some point.",
    words: words(
      "Someone should really look into the CRM migration at some point",
    ),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [],
  },
  {
    name: "implied-owner-from-speaker",
    transcript: "Bob: I'll book the offsite venue and confirm catering.",
    words: words("I'll book the offsite venue and confirm catering", 5000),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [
      { source_fragment: "book the offsite venue", owner_speaker_id: "spk_2" },
    ],
  },
  {
    name: "explicit-iso-date",
    transcript:
      "Alice: Let's finalize the deck. I'll have it ready by 2026-08-03.",
    words: words(
      "Let's finalize the deck I'll have it ready by 2026-08-03",
      8000,
    ),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [
      {
        source_fragment: "finalize the deck",
        owner_speaker_id: "spk_1",
        due_at: "2026-08-03",
      },
    ],
  },
  {
    name: "unassigned-task-null-owner",
    // A real task but no clear owner -> item kept with null owner.
    transcript:
      "Alice: We need to renew the SSL certificate before it expires.",
    words: words(
      "We need to renew the SSL certificate before it expires",
      12000,
    ),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [
      { source_fragment: "renew the SSL certificate", owner_speaker_id: "" },
    ],
  },
  {
    name: "cross-talk-two-owners",
    transcript:
      "Alice: I'll update the roadmap doc. Bob: And I'll email the client the new timeline tomorrow.",
    words: words(
      "I'll update the roadmap doc And I'll email the client the new timeline tomorrow",
      15000,
    ),
    roster: R2,
    meetingDate: "2026-07-23", // tomorrow = 2026-07-24
    expected: [
      { source_fragment: "update the roadmap doc", owner_speaker_id: "spk_1" },
      {
        source_fragment: "email the client the new timeline",
        owner_speaker_id: "spk_2",
        due_at: "2026-07-24",
      },
    ],
  },
  {
    name: "question-not-a-task",
    transcript: "Bob: Do we have budget approval for the new hires yet?",
    words: words("Do we have budget approval for the new hires yet"),
    roster: R2,
    meetingDate: "2026-07-23",
    expected: [],
  },
];
