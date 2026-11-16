import { useMemo } from "react";

import { md2json } from "@hypr/editor/markdown";

import { EVENT_SERIES_ID_SQL, seriesIdForEvent } from "./identity";

import { liveQueryClient, useLiveQuery } from "~/db";
import { createSession, getOrCreateSessionForEventId } from "~/session/queries";
import { DEFAULT_USER_ID } from "~/shared/utils";

export type SeriesOccurrence = {
  id: string;
  title: string;
  started_at: string;
  ended_at: string;
  created_at: string;
  date: string;
  duration_minutes: number;
  tldr: string;
  enhanced_note: string;
  open_item_count: number;
  event_json: string;
};

export type SeriesOccurrenceSqlRow = {
  id: string;
  title: string | null;
  started_at: string | null;
  ended_at: string | null;
  created_at: string;
  event_json: string | null;
  enhanced_note: string | null;
  open_item_count: number | null;
};

export type SeriesCarryOver = {
  id: string;
  session_id: string;
  text: string;
  assignee_human_id: string;
  assignee_name: string;
  due_at: string;
  carried_from_session_id: string | null;
  source_title: string;
  source_started_at: string;
};

export type SeriesDelta = {
  decisions: Array<{ text: string; owner?: string }>;
  actionsClosed: Array<{ id: string; text: string }>;
  attendeesJoined: Array<{ name: string; email?: string }>;
  attendeesLeft: Array<{ name: string; email?: string }>;
  topicsAdded: string[];
  topicsRemoved: string[];
};

export type SeriesEvent = {
  id: string;
  tracking_id_event: string;
  calendar_id: string;
  title: string;
  started_at: string;
  ended_at: string;
  location: string;
  meeting_link: string;
  description: string;
  recurrence_series_id: string;
  has_recurrence_rules: number;
  is_all_day: number;
  provider: string;
  participants_json: string;
};

export const SERIES_SQL = `
  SELECT
    s.id,
    COALESCE(NULLIF(s.title, ''), '') AS title,
    s.started_at,
    s.ended_at,
    s.created_at,
    COALESCE(s.event_json, '') AS event_json,
    COALESCE((
      SELECT group_concat(document.body, char(10))
      FROM session_documents AS document
      WHERE document.session_id = s.id
        AND document.kind = 'enhanced'
        AND document.deleted_at IS NULL
    ), '') AS enhanced_note,
    (SELECT COUNT(*) FROM action_items AS item
      WHERE item.session_id = s.id AND item.status <> 'done' AND item.deleted_at IS NULL
    ) AS open_item_count
  FROM sessions AS s
  WHERE s.series_id = ? AND s.deleted_at IS NULL
  ORDER BY COALESCE(NULLIF(s.started_at, ''), s.created_at) DESC
`;

export function extractTldr(enhancedNote?: string | null): string {
  if (!enhancedNote) return "";
  const stripped = enhancedNote.replace(/```[\s\S]*?```/g, "");
  const line = stripped
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => l.length > 0);
  if (!line) return "";
  return line.replace(/^#+\s*/, "").replace(/^[-*]\s*/, "");
}

export function computeDurationMinutes(
  startedAt?: string | null,
  endedAt?: string | null,
): number {
  if (!startedAt || !endedAt) return 0;
  const start = new Date(startedAt).getTime();
  const end = new Date(endedAt).getTime();
  if (Number.isNaN(start) || Number.isNaN(end) || end <= start) return 0;
  return Math.round((end - start) / 60000);
}

export function mapOccurrenceRow(
  row: SeriesOccurrenceSqlRow,
): SeriesOccurrence {
  const startedAt = row.started_at ?? "";
  const endedAt = row.ended_at ?? "";
  const createdAt = row.created_at ?? "";
  const enhancedNote = row.enhanced_note ?? "";
  return {
    id: row.id,
    title: row.title ?? "",
    started_at: startedAt,
    ended_at: endedAt,
    created_at: createdAt,
    date: startedAt || createdAt,
    duration_minutes: computeDurationMinutes(startedAt, endedAt),
    tldr: extractTldr(enhancedNote),
    enhanced_note: enhancedNote,
    open_item_count: Number(row.open_item_count ?? 0),
    event_json: row.event_json ?? "",
  };
}

export function useSeries(seriesId: string) {
  return useLiveQuery<SeriesOccurrenceSqlRow, SeriesOccurrence[]>({
    sql: SERIES_SQL,
    params: [seriesId],
    mapRows: (rows) => rows.map(mapOccurrenceRow),
    enabled: Boolean(seriesId),
  });
}

// These are the original task rows, not copies owned by later occurrences.
// Completing one here completes the same task shown in its source session.
export const SERIES_OPEN_ITEMS_SQL = `
  SELECT item.id, item.session_id, item.text, item.assignee_human_id,
    item.due_at, item.carried_from_session_id,
    session.title AS source_title,
    COALESCE(NULLIF(session.started_at, ''), session.created_at) AS source_started_at,
    COALESCE(NULLIF(h.name, ''), item.owner_speaker_id, '') AS assignee_name
  FROM action_items AS item
  JOIN sessions AS session ON session.id = item.session_id
  LEFT JOIN humans AS h ON h.id = item.assignee_human_id
  WHERE session.series_id = ? AND session.deleted_at IS NULL
    AND item.status <> 'done' AND item.deleted_at IS NULL
`;

export function useSeriesCarryOver(seriesId: string, beforeSessionId?: string) {
  const sql =
    SERIES_OPEN_ITEMS_SQL +
    (beforeSessionId
      ? `
    AND COALESCE(NULLIF(session.started_at, ''), session.created_at) < (
      SELECT COALESCE(NULLIF(started_at, ''), created_at)
      FROM sessions WHERE id = ? AND deleted_at IS NULL
    )
  `
      : "") +
    `
    ORDER BY COALESCE(NULLIF(session.started_at, ''), session.created_at) DESC,
      item.created_at ASC, item.id
  `;
  return useLiveQuery<SeriesCarryOver, SeriesCarryOver[]>({
    sql,
    params: beforeSessionId ? [seriesId, beforeSessionId] : [seriesId],
    enabled: Boolean(seriesId),
  });
}

export function extractTopics(note?: string | null): string[] {
  if (!note) return [];
  return [...note.matchAll(/^##+\s+(.+)$/gm)].map((match) => match[1].trim());
}

export function parseDecisionsFromNote(
  note?: string | null,
): Array<{ text: string; owner?: string }> {
  if (!note) return [];
  const fenced = note.match(/```decisions\s*\n([\s\S]*?)\n```/);
  if (fenced?.[1]) {
    try {
      const parsed: unknown = JSON.parse(fenced[1]);
      if (Array.isArray(parsed)) {
        const decisions: Array<{ text: string; owner?: string }> = [];
        for (const item of parsed) {
          if (item && typeof item === "object") {
            const text =
              "text" in item && typeof item.text === "string"
                ? item.text
                : String(item);
            const owner =
              "owner" in item && typeof item.owner === "string"
                ? item.owner
                : undefined;
            decisions.push({ text, owner });
          } else if (typeof item === "string") {
            decisions.push({ text: item });
          }
        }
        return decisions;
      }
    } catch {
      // ignore
    }
  }
  return [];
}

export function parseParticipants(
  eventJson?: string | null,
): Array<{ name: string; email?: string }> {
  if (!eventJson) return [];
  try {
    const parsed: unknown = JSON.parse(eventJson);
    if (
      parsed &&
      typeof parsed === "object" &&
      "participants" in parsed &&
      Array.isArray(parsed.participants)
    ) {
      const result: Array<{ name: string; email?: string }> = [];
      for (const p of parsed.participants) {
        if (p && typeof p === "object") {
          const name =
            "name" in p && typeof p.name === "string"
              ? p.name
              : "display_name" in p && typeof p.display_name === "string"
                ? p.display_name
                : "email" in p && typeof p.email === "string"
                  ? p.email
                  : "";
          const email =
            "email" in p && typeof p.email === "string" ? p.email : undefined;
          if (name) {
            result.push({ name, email });
          }
        }
      }
      return result;
    }
  } catch {
    // ignore
  }
  return [];
}

export function useSeriesDelta(
  first?: string | Partial<SeriesOccurrence>,
  second?: string | Partial<SeriesOccurrence>,
  third?: string,
): SeriesDelta {
  const isStringCall = typeof first === "string";
  const currentId = isStringCall ? (second as string | undefined) : first?.id;
  const previousId = isStringCall
    ? third
    : (second as Partial<SeriesOccurrence> | undefined)?.id;

  const currentOccurrence = isStringCall
    ? undefined
    : (first as Partial<SeriesOccurrence> | undefined);
  const previousOccurrence = isStringCall
    ? undefined
    : (second as Partial<SeriesOccurrence> | undefined);

  // Check if session_decisions table exists
  const { data: tableCheck = [] } = useLiveQuery<
    { name: string },
    { name: string }[]
  >({
    sql: "SELECT name FROM sqlite_master WHERE type='table' AND name='session_decisions'",
  });
  const hasDecisionsTable = tableCheck.length > 0;

  // Query decisions from session_decisions table if it exists
  const { data: dbDecisions = [] } = useLiveQuery<
    { text: string; owner: string | null },
    { text: string; owner: string | null }[]
  >({
    sql: "SELECT text, owner FROM session_decisions WHERE session_id = ? AND deleted_at IS NULL",
    params: [currentId ?? ""],
    enabled: Boolean(hasDecisionsTable && currentId),
  });

  // Query action items closed in current session
  const { data: closedActions = [] } = useLiveQuery<
    { id: string; text: string },
    { id: string; text: string }[]
  >({
    sql: `
      SELECT id, text FROM action_items
      WHERE session_id = ? AND status = 'done' AND deleted_at IS NULL
      ORDER BY updated_at DESC
    `,
    params: [currentId ?? ""],
    enabled: Boolean(currentId),
  });

  // Query session_participants for current and previous
  const { data: currentDbParticipants = [] } = useLiveQuery<
    { display_name: string; email: string },
    { display_name: string; email: string }[]
  >({
    sql: "SELECT display_name, email FROM session_participants WHERE session_id = ? AND deleted_at IS NULL",
    params: [currentId ?? ""],
    enabled: Boolean(currentId),
  });

  const { data: previousDbParticipants = [] } = useLiveQuery<
    { display_name: string; email: string },
    { display_name: string; email: string }[]
  >({
    sql: "SELECT display_name, email FROM session_participants WHERE session_id = ? AND deleted_at IS NULL",
    params: [previousId ?? ""],
    enabled: Boolean(previousId),
  });

  return useMemo(() => {
    // 1. Decisions
    const noteDecisions = parseDecisionsFromNote(
      currentOccurrence?.enhanced_note,
    );
    const combinedDecisions =
      dbDecisions.length > 0
        ? dbDecisions.map((d) => ({
            text: d.text,
            owner: d.owner ?? undefined,
          }))
        : noteDecisions;

    // 2. Action items closed
    const actionsClosed = closedActions.map((a) => ({
      id: a.id,
      text: a.text,
    }));

    // 3. Attendees joined / left
    const currentPeople = new Map<string, { name: string; email?: string }>();
    const previousPeople = new Map<string, { name: string; email?: string }>();

    for (const p of parseParticipants(currentOccurrence?.event_json)) {
      currentPeople.set((p.email || p.name).toLowerCase(), p);
    }
    for (const p of currentDbParticipants) {
      const name = p.display_name || p.email;
      if (name)
        currentPeople.set((p.email || name).toLowerCase(), {
          name,
          email: p.email,
        });
    }

    for (const p of parseParticipants(previousOccurrence?.event_json)) {
      previousPeople.set((p.email || p.name).toLowerCase(), p);
    }
    for (const p of previousDbParticipants) {
      const name = p.display_name || p.email;
      if (name)
        previousPeople.set((p.email || name).toLowerCase(), {
          name,
          email: p.email,
        });
    }

    const attendeesJoined: Array<{ name: string; email?: string }> = [];
    for (const [key, person] of currentPeople) {
      if (!previousPeople.has(key)) {
        attendeesJoined.push(person);
      }
    }

    const attendeesLeft: Array<{ name: string; email?: string }> = [];
    for (const [key, person] of previousPeople) {
      if (!currentPeople.has(key)) {
        attendeesLeft.push(person);
      }
    }

    // 4. Topics added / removed
    const currentTopics = extractTopics(currentOccurrence?.enhanced_note);
    const previousTopics = extractTopics(previousOccurrence?.enhanced_note);

    const topicsAdded = currentTopics.filter(
      (t) => !previousTopics.includes(t),
    );
    const topicsRemoved = previousTopics.filter(
      (t) => !currentTopics.includes(t),
    );

    return {
      decisions: combinedDecisions,
      actionsClosed,
      attendeesJoined,
      attendeesLeft,
      topicsAdded,
      topicsRemoved,
    };
  }, [
    currentOccurrence?.enhanced_note,
    previousOccurrence?.enhanced_note,
    currentOccurrence?.event_json,
    previousOccurrence?.event_json,
    dbDecisions,
    closedActions,
    currentDbParticipants,
    previousDbParticipants,
  ]);
}

export async function prepareSeriesOccurrence(
  seriesId: string,
  eventId?: string,
): Promise<string> {
  const rows = await liveQueryClient.execute<SeriesOccurrenceSqlRow>(
    SERIES_SQL,
    [seriesId],
  );
  const latest = rows[0] ? mapOccurrenceRow(rows[0]) : undefined;
  const eventColumns = `id, tracking_id_event, calendar_id, title, started_at,
    ended_at, location, meeting_link, description, recurrence_series_id,
    has_recurrence_rules, is_all_day, provider, participants_json`;
  let event: SeriesEvent | undefined;
  if (eventId) {
    [event] = await liveQueryClient.execute<SeriesEvent>(
      `SELECT ${eventColumns} FROM events
       WHERE id = ? AND deleted_at IS NULL LIMIT 1`,
      [eventId],
    );
    if (!event || seriesIdForEvent(event) !== seriesId) {
      throw new Error(`Event ${eventId} does not belong to series ${seriesId}`);
    }
  } else {
    [event] = await liveQueryClient.execute<SeriesEvent>(
      `SELECT ${eventColumns} FROM events
       WHERE (${EVENT_SERIES_ID_SQL}) = ? AND deleted_at IS NULL
         ${latest ? "AND started_at > ?" : ""}
       ORDER BY started_at ASC, id LIMIT 1`,
      latest ? [seriesId, latest.started_at || latest.created_at] : [seriesId],
    );
  }

  const previous = event
    ? rows.map(mapOccurrenceRow).find((row) => row.date < event.started_at)
    : latest;
  if (!previous && !event) {
    throw new Error(`No occurrence or event found for series ${seriesId}`);
  }

  // Carry over provenance only. The Series view owns the live checklist and
  // references each original action id, so completion never forks across notes.
  const items = await liveQueryClient.execute<SeriesCarryOver>(
    SERIES_OPEN_ITEMS_SQL +
      (event
        ? `
      AND COALESCE(NULLIF(session.started_at, ''), session.created_at) < ?
    `
        : "") +
      " ORDER BY item.created_at, item.id",
    event ? [seriesId, event.started_at] : [seriesId],
  );
  const carriedBlockLines = ["> ## From last time"];
  if (previous?.enhanced_note) {
    const tldr = extractTldr(previous.enhanced_note);
    if (tldr) carriedBlockLines.push(`> Last meeting: ${tldr}`);
  }
  for (const item of items) {
    // Escape source text so it cannot turn provenance into task/list syntax.
    const text = item.text
      .replace(/\s+/g, " ")
      .replace(/[\\`*_[\]<>]/g, "\\$&");
    carriedBlockLines.push(`> - ${text}`);
  }
  if (items.length === 0 && !previous?.enhanced_note) {
    carriedBlockLines.push("> No open items carried over.");
  }
  const initialRawMd = JSON.stringify(
    md2json(carriedBlockLines.join("\n") + "\n\n"),
  );
  const title = event?.title || previous?.title || "Recurring meeting";

  // Initialization is in the same transaction as the conditional event-session
  // insert. A losing concurrent request never updates an existing note body.
  if (event) {
    return getOrCreateSessionForEventId(
      event.id,
      title,
      DEFAULT_USER_ID,
      initialRawMd,
    );
  }
  return createSession(title, DEFAULT_USER_ID, {
    series_id: seriesId,
    raw_md: initialRawMd,
  });
}
