import { useQuery } from "@tanstack/react-query";

import {
  commands as calendarCommands,
  type CalendarProviderType,
} from "@hypr/plugin-calendar";

import type { FollowupAttendee } from "./schedule-followup-defaults";

export type WritableCalendarTarget = {
  key: string;
  provider: Extract<CalendarProviderType, "google" | "outlook">;
  connectionId: string;
  calendarTrackingId: string;
  calendarTitle: string;
  accountEmail: string;
};

export const WRITABLE_CALENDARS_QUERY_KEY = [
  "schedule-followup-writable-calendars",
];

/** `google:<email>` / `outlook:<email>` -> `<email>` (falls back to the raw id). */
export function accountEmailFromConnectionId(connectionId: string): string {
  const idx = connectionId.indexOf(":");
  const raw = idx === -1 ? connectionId : connectionId.slice(idx + 1);
  // Lowercase for consistency with `defaultFollowupAttendees`'s dedupe key,
  // even though providers already canonicalize account email casing.
  return raw.toLowerCase();
}

/**
 * Writable (can_edit) calendars across every connected Google/Outlook
 * account. Apple and `.ics` calendars never expose a write API, so they're
 * excluded. Hits the live provider APIs (not the local synced inventory,
 * which doesn't carry `can_edit`), so callers should cache this.
 */
export async function fetchWritableCalendarTargets(): Promise<
  WritableCalendarTarget[]
> {
  const connectionsResult = await calendarCommands.listConnectionIds();
  if (connectionsResult.status === "error") return [];

  const targets: WritableCalendarTarget[] = [];

  for (const { provider, connection_ids } of connectionsResult.data) {
    if (provider !== "google" && provider !== "outlook") continue;

    for (const connectionId of connection_ids) {
      const calendarsResult = await calendarCommands.listCalendars(
        provider,
        connectionId,
      );
      if (calendarsResult.status === "error") continue;

      for (const calendar of calendarsResult.data) {
        if (calendar.can_edit !== true) continue;
        targets.push({
          key: `${provider}::${connectionId}::${calendar.id}`,
          provider,
          connectionId,
          calendarTrackingId: calendar.id,
          calendarTitle: calendar.title,
          accountEmail: accountEmailFromConnectionId(connectionId),
        });
      }
    }
  }

  return targets;
}

export function useWritableCalendarTargets() {
  return useQuery({
    queryKey: WRITABLE_CALENDARS_QUERY_KEY,
    queryFn: fetchWritableCalendarTargets,
    staleTime: 5 * 60 * 1000,
  });
}

/** Creates the follow-up event on the chosen calendar; throws on failure. */
export async function submitFollowupEvent({
  target,
  title,
  startIso,
  endIso,
  description,
  attendees,
}: {
  target: WritableCalendarTarget;
  title: string;
  startIso: string;
  endIso: string;
  description: string;
  attendees: FollowupAttendee[];
}): Promise<string> {
  const result = await calendarCommands.createEvent(
    target.provider,
    target.connectionId,
    {
      calendar_tracking_id: target.calendarTrackingId,
      title,
      started_at: startIso,
      ended_at: endIso,
      is_all_day: false,
      location: null,
      notes: description.trim() ? description.trim() : null,
      url: null,
      attendees:
        attendees.length > 0
          ? attendees.map((attendee) => ({
              email: attendee.email,
              name: attendee.name || null,
            }))
          : null,
    },
  );

  if (result.status === "error") {
    throw new Error(result.error);
  }

  return result.data;
}
