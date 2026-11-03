import { format, safeParseDate } from "@hypr/utils";

export const DEFAULT_DURATION_MINUTES = 30;
export const DURATION_OPTIONS_MINUTES = [15, 30, 45, 60] as const;

/**
 * Rolls a date forward to the next business day (Mon–Fri), preserving the
 * time-of-day. Always advances at least one calendar day, then skips
 * Saturday/Sunday forward to the following Monday.
 */
export function nextBusinessDay(date: Date): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + 1);
  const day = next.getDay();
  if (day === 6) {
    next.setDate(next.getDate() + 2);
  } else if (day === 0) {
    next.setDate(next.getDate() + 1);
  }
  return next;
}

/** Default follow-up start: next business day, same time as `now`. */
export function defaultFollowupStart(now: Date = new Date()): Date {
  return nextBusinessDay(now);
}

/** Default follow-up title: "<meeting title> — follow-up". */
export function defaultFollowupTitle(
  meetingTitle: string | null | undefined,
): string {
  const base = (meetingTitle ?? "").trim() || "Meeting";
  return `${base} — follow-up`;
}

/** Local wall-clock value for an `<input type="datetime-local">`. */
export function toDatetimeLocalValue(date: Date): string {
  return format(date, "yyyy-MM-dd'T'HH:mm");
}

/** Parses a `datetime-local` value (local wall-clock) to a UTC ISO string. */
export function datetimeLocalToIso(value: string): string | null {
  const parsed = safeParseDate(value);
  return parsed ? parsed.toISOString() : null;
}

/** Adds `minutes` to an ISO timestamp, returning a new ISO timestamp. */
export function addMinutesIso(
  startIso: string,
  minutes: number,
): string | null {
  const start = safeParseDate(startIso);
  if (!start) return null;
  return new Date(start.getTime() + minutes * 60_000).toISOString();
}

export type FollowupActionItem = { text: string; status: string };

function isOpenActionItem(item: FollowupActionItem): boolean {
  return (
    item.status !== "done" &&
    item.status !== "completed" &&
    item.text.trim().length > 0
  );
}

/** Default description: the note's open (not-done) action items, bulleted. */
export function defaultFollowupDescription(
  actionItems: FollowupActionItem[],
): string {
  const open = actionItems.filter(isOpenActionItem);
  if (open.length === 0) return "";
  return open.map((item) => `- ${item.text.trim()}`).join("\n");
}

export type FollowupParticipant = {
  humanId: string;
  name: string;
  email: string;
};

export type FollowupAttendee = { id: string; name: string; email: string };

/** Default attendees: session participants that have an email, deduped. */
export function defaultFollowupAttendees(
  participants: FollowupParticipant[],
): FollowupAttendee[] {
  const seen = new Set<string>();
  const attendees: FollowupAttendee[] = [];

  for (const participant of participants) {
    const email = participant.email?.trim().toLowerCase();
    if (!email || !email.includes("@") || seen.has(email)) continue;
    seen.add(email);
    attendees.push({
      id: participant.humanId || email,
      name: participant.name?.trim() || email,
      email,
    });
  }

  return attendees;
}
