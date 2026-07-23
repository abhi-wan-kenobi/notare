/**
 * Structural anti-hallucination gates for action-item extraction (WS-C).
 *
 * The product guarantee is enforced HERE, in code — never by trusting the
 * model's prompt compliance:
 *
 *  1. `source_text` MUST be a verbatim substring of the (normalized) meeting
 *     transcript. An item whose source_text is not found is REJECTED outright.
 *     This makes a fabricated action item structurally impossible to keep: the
 *     model cannot invent a task without also inventing a transcript quote,
 *     and an invented quote fails the substring check. Release gate:
 *     hallucinated source_text = 0.
 *  2. `owner_speaker_id` must be a member of the closed speaker roster, else it
 *     is cleared to "" (null owner). The model never gets to invent an owner.
 *  3. `due_at` is re-parsed from the model's raw value against the meeting date;
 *     anything that doesn't resolve to a real date becomes "".
 *  4. `source_start_ms` is located by matching source_text against the
 *     transcript's word timings — not taken from the model.
 *
 * These functions are pure so the guarantee is unit-testable independently of
 * any model.
 */

export type SpeakerRosterEntry = {
  /** Diarization speaker id (matches Word.speaker / speaker_hints owner). */
  speakerId: string;
  /** Optional human-readable label for the prompt (e.g. "Alice"). */
  label?: string;
};

export type TranscriptWord = {
  text: string;
  start_ms: number;
  end_ms: number;
  speaker?: string;
};

/** Raw item as returned by the model (pre-gate). */
export type RawActionItem = {
  text: string;
  source_text: string;
  owner_speaker_id?: string | null;
  due_at?: string | null;
  priority?: string | null;
  confidence?: number | null;
};

/** Item after all gates pass; ready to persist against action_items v2. */
export type GatedActionItem = {
  text: string;
  source_text: string;
  owner_speaker_id: string;
  due_at: string;
  source_start_ms: number | null;
  priority: string;
  confidence: number;
};

export type GateRejection = {
  item: RawActionItem;
  reason: "empty_text" | "empty_source_text" | "source_text_not_in_transcript";
};

export type GateResult = {
  kept: GatedActionItem[];
  rejected: GateRejection[];
};

/**
 * Normalize text for substring matching: lowercase, strip anything that isn't
 * a letter/number/space (punctuation, quotes the model may add/drop), collapse
 * runs of whitespace, trim. Applied identically to the transcript and to
 * source_text so the comparison is robust to cosmetic differences while still
 * requiring the words to actually be present and in order.
 */
export function normalizeForMatch(s: string): string {
  return s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** Gate 1: verbatim-source substring check. */
export function sourceTextIsInTranscript(
  sourceText: string,
  normalizedTranscript: string,
): boolean {
  const needle = normalizeForMatch(sourceText);
  if (!needle) return false;
  return normalizedTranscript.includes(needle);
}

/** Gate 2: owner must be a roster member, else null (""). */
export function resolveOwner(
  ownerSpeakerId: string | null | undefined,
  roster: SpeakerRosterEntry[],
): string {
  if (!ownerSpeakerId) return "";
  return roster.some((r) => r.speakerId === ownerSpeakerId)
    ? ownerSpeakerId
    : "";
}

const ISO_DATE_RE =
  /^\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?Z?)?$/;
const WEEKDAYS = [
  "sunday",
  "monday",
  "tuesday",
  "wednesday",
  "thursday",
  "friday",
  "saturday",
];

/**
 * Gate 3: resolve `raw` to an ISO date (YYYY-MM-DD) relative to `meetingDate`,
 * or "" if it cannot be sanely parsed. Deliberately conservative — it accepts
 * ISO dates, and a small set of unambiguous relative forms the model reliably
 * emits ("today", "tomorrow", "next monday", "in 3 days", "next week"). Vague
 * phrases ("soon", "later", "asap") resolve to "" rather than a guessed date.
 */
export function resolveDueDate(
  raw: string | null | undefined,
  meetingDate: Date,
): string {
  if (!raw) return "";
  const value = raw.trim().toLowerCase();
  if (!value) return "";

  if (ISO_DATE_RE.test(raw.trim())) {
    const d = new Date(
      raw.trim().length <= 10 ? `${raw.trim()}T00:00:00Z` : raw.trim(),
    );
    return Number.isNaN(d.getTime()) ? "" : toIsoDate(d);
  }

  const base = new Date(
    Date.UTC(
      meetingDate.getUTCFullYear(),
      meetingDate.getUTCMonth(),
      meetingDate.getUTCDate(),
    ),
  );

  if (value === "today") return toIsoDate(base);
  if (value === "tomorrow") return toIsoDate(addDays(base, 1));
  if (value === "yesterday") return ""; // a due date in the past is not sane

  const inDays = /^in (\d{1,3}) days?$/.exec(value);
  if (inDays) return toIsoDate(addDays(base, Number(inDays[1])));

  if (value === "next week") return toIsoDate(addDays(base, 7));

  const nextWeekday =
    /^(?:next |this )?(sunday|monday|tuesday|wednesday|thursday|friday|saturday)$/.exec(
      value,
    );
  if (nextWeekday) {
    const target = WEEKDAYS.indexOf(nextWeekday[1]);
    const cur = base.getUTCDay();
    let delta = (target - cur + 7) % 7;
    if (delta === 0) delta = 7; // "next monday" on a monday => the following one
    return toIsoDate(addDays(base, delta));
  }

  return "";
}

function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setUTCDate(r.getUTCDate() + n);
  return r;
}

function toIsoDate(d: Date): string {
  return d.toISOString().slice(0, 10);
}

/**
 * Gate 4: find the start_ms of the transcript span that produced `sourceText`,
 * by locating the first word window whose normalized concatenation contains the
 * normalized source_text. Returns null when the words can't be aligned (e.g.
 * the transcript has no word timings). Never fabricates a timestamp.
 */
export function locateSourceStartMs(
  sourceText: string,
  words: TranscriptWord[],
): number | null {
  const needle = normalizeForMatch(sourceText);
  if (!needle || words.length === 0) return null;

  const normWords = words.map((w) => normalizeForMatch(w.text));
  // A verbatim quote begins at a word boundary, so anchor at each word i and
  // grow the window until it's long enough to contain the needle, then require
  // it to START WITH the needle — that guarantees `words[i]` is the first word
  // of the match (not merely a window that contains the needle further in).
  for (let i = 0; i < words.length; i++) {
    let joined = "";
    for (let j = i; j < words.length; j++) {
      joined = joined ? `${joined} ${normWords[j]}` : normWords[j];
      if (joined.length >= needle.length) {
        if (joined.startsWith(needle)) return words[i].start_ms;
        break; // this anchor can't match; try the next word
      }
    }
  }
  return null;
}

/**
 * Run every gate over the model's raw items. `transcript` is the full meeting
 * transcript text; `words` its per-word timings; `roster` the closed speaker
 * set; `meetingDate` the absolute meeting date for relative due-date parsing.
 */
export function applyGates(
  rawItems: RawActionItem[],
  params: {
    transcript: string;
    words: TranscriptWord[];
    roster: SpeakerRosterEntry[];
    meetingDate: Date;
  },
): GateResult {
  const normalizedTranscript = normalizeForMatch(params.transcript);
  const kept: GatedActionItem[] = [];
  const rejected: GateRejection[] = [];

  for (const item of rawItems) {
    if (!item.text?.trim()) {
      rejected.push({ item, reason: "empty_text" });
      continue;
    }
    if (!item.source_text?.trim()) {
      rejected.push({ item, reason: "empty_source_text" });
      continue;
    }
    if (!sourceTextIsInTranscript(item.source_text, normalizedTranscript)) {
      rejected.push({ item, reason: "source_text_not_in_transcript" });
      continue;
    }

    kept.push({
      text: item.text.trim(),
      source_text: item.source_text.trim(),
      owner_speaker_id: resolveOwner(item.owner_speaker_id, params.roster),
      due_at: resolveDueDate(item.due_at, params.meetingDate),
      source_start_ms: locateSourceStartMs(item.source_text, params.words),
      priority: normalizePriority(item.priority),
      confidence: clampConfidence(item.confidence),
    });
  }

  return { kept, rejected };
}

function normalizePriority(p: string | null | undefined): string {
  const v = (p ?? "").trim().toLowerCase();
  return v === "low" || v === "medium" || v === "high" ? v : "";
}

function clampConfidence(c: number | null | undefined): number {
  if (typeof c !== "number" || Number.isNaN(c)) return 0;
  return Math.min(1, Math.max(0, c));
}
