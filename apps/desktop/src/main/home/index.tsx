import { Trans, useLingui } from "@lingui/react/macro";
import {
  addDays,
  endOfWeek,
  format,
  formatDistance,
  startOfDay,
  startOfWeek,
} from "date-fns";
import { RepeatIcon } from "lucide-react";
import { useMemo } from "react";

import { toTz, useNow, useTimezone, useWeekStartsOn } from "~/calendar/hooks";
import {
  parseEventParticipants,
  useTimelineEventsTable,
} from "~/calendar/queries";
import { useLiveQuery } from "~/db";
import {
  getOrCreateSessionForEventId,
  useSessionSummaries,
} from "~/session/queries";
import { Dock } from "~/shared/dock";
import { StandardContentWrapper } from "~/shared/main";
import { openSurfaceTab } from "~/sidebar/surface-navigation";
import type { TimelineEventsTable } from "~/sidebar/timeline/utils";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const COMING_UP_LIMIT = 4;
const RECENT_NOTES_LIMIT = 5;

type SessionLink = {
  session_id: string;
  event_id: string;
  external_event_id: string;
  series_id?: string;
};

type EventParticipantsSqlRow = {
  id: string;
  participants_json: string | null;
};

const EMPTY_SESSION_LINKS: SessionLink[] = [];
const EMPTY_ATTENDEES: Record<string, string[]> = {};

export function TabContentHome({
  tab: _tab,
}: {
  tab: Extract<Tab, { type: "empty" }>;
}) {
  return (
    <StandardContentWrapper noBorder>
      <HomeView />
    </StandardContentWrapper>
  );
}

function useSessionsNeedingSummary(): number {
  const { data = 0 } = useLiveQuery<{ count: number }, number>({
    sql: `
      SELECT COUNT(*) AS count
      FROM sessions AS session
      WHERE session.deleted_at IS NULL
        AND EXISTS (
          SELECT 1
          FROM transcripts AS transcript
          WHERE transcript.session_id = session.id
            AND transcript.deleted_at IS NULL
            AND CASE
              WHEN json_valid(transcript.words_json)
              THEN json_array_length(transcript.words_json)
              ELSE 0
            END > 0
        )
        AND NOT EXISTS (
          SELECT 1
          FROM session_documents AS document
          WHERE document.session_id = session.id
            AND document.kind IN ('summary', 'template_output', 'enhanced_note')
            AND document.deleted_at IS NULL
        )
    `,
    mapRows: (rows) => Number(rows[0]?.count ?? 0),
  });
  return data;
}

type RecurrenceCadence = "weekly" | "biweekly" | "monthly" | "unknown";

function HomeView() {
  const { t } = useLingui();
  const tz = useTimezone();
  const now = useNow();
  const today = toTz(now, tz);
  const todayKey = format(today, "yyyy-MM-dd");

  const eventsTable = useTimelineEventsTable();
  const sessions = useSessionSummaries();
  const openCurrent = useTabs((state) => state.openCurrent);

  const { data: sessionLinks = EMPTY_SESSION_LINKS } = useLiveQuery<
    SessionLink,
    SessionLink[]
  >({
    sql: `
      SELECT id AS session_id, event_id, external_event_id, series_id
      FROM sessions
      WHERE deleted_at IS NULL
        AND ((event_id IS NOT NULL AND event_id != '')
          OR (external_event_id IS NOT NULL AND external_event_id != '')
          OR (series_id IS NOT NULL AND series_id != ''))
    `,
  });

  const { data: attendeesByEventId = EMPTY_ATTENDEES } = useLiveQuery<
    EventParticipantsSqlRow,
    Record<string, string[]>
  >({
    sql: `
      SELECT id, participants_json
      FROM events
      WHERE deleted_at IS NULL
    `,
    mapRows: (rows) =>
      Object.fromEntries(
        rows.map(({ id, participants_json }) => [
          id,
          parseEventParticipants(participants_json ?? undefined)
            .filter((participant) => participant.is_current_user !== true)
            .map(
              (participant) =>
                participant.name?.trim() || participant.email?.trim() || "",
            )
            .filter(Boolean),
        ]),
      ),
  });

  const { data: openActionItemsCount = 0 } = useLiveQuery<
    { count: number },
    number
  >({
    sql: `
      SELECT COUNT(*) AS count
      FROM action_items
      WHERE deleted_at IS NULL AND status != 'done'
    `,
    mapRows: (rows) => Number(rows[0]?.count ?? 0),
  });

  const sessionsNeedingSummaryCount = useSessionsNeedingSummary();

  const sessionIdByEventId = useMemo(() => {
    const byEventId = new Map<string, string>();
    const byTrackingId = new Map<string, string>();
    for (const link of sessionLinks) {
      if (link.event_id) byEventId.set(link.event_id, link.session_id);
      if (link.external_event_id) {
        byTrackingId.set(link.external_event_id, link.session_id);
      }
    }
    return { byEventId, byTrackingId };
  }, [sessionLinks]);

  const weekStartsOn = useWeekStartsOn();
  const comingUp = useMemo(() => {
    if (!eventsTable) return [];
    const tomorrowStart = startOfDay(addDays(today, 1));
    const weekEnd = endOfWeek(today, { weekStartsOn });

    return Object.entries(eventsTable)
      .filter(([, event]) => event.title && event.started_at)
      .filter(([, event]) => {
        const startedAt = toTz(event.started_at as string, tz);
        return (
          startedAt.getTime() >= now.getTime() &&
          startedAt.getTime() <= weekEnd.getTime()
        );
      })
      .sort((a, b) =>
        String(a[1].started_at).localeCompare(String(b[1].started_at)),
      )
      .slice(0, COMING_UP_LIMIT)
      .map(([id, event]) => {
        const eventDate = toTz(event.started_at as string, tz);
        const dayKey = format(eventDate, "yyyy-MM-dd");
        return {
          id,
          title: event.title as string,
          startedAt: event.started_at as string,
          endedAt: event.ended_at || undefined,
          isRecurring: Boolean(event.recurrence_series_id),
          seriesId: (event.recurrence_series_id as string) || undefined,
          dayLabel:
            dayKey === todayKey
              ? "today"
              : dayKey === format(tomorrowStart, "yyyy-MM-dd")
                ? "tomorrow"
                : "later",
          sessionId:
            sessionIdByEventId.byEventId.get(id) ??
            (event.tracking_id_event
              ? sessionIdByEventId.byTrackingId.get(
                  event.tracking_id_event as string,
                )
              : undefined),
          attendees: attendeesByEventId[id] ?? [],
          cadence: getRecurrenceCadence(
            eventsTable,
            event.recurrence_series_id,
          ),
        };
      });
  }, [
    attendeesByEventId,
    eventsTable,
    now,
    sessionIdByEventId,
    today,
    todayKey,
    tz,
    weekStartsOn,
  ]);

  const scheduledTodayCount = useMemo(() => {
    if (!eventsTable) return 0;
    return Object.values(eventsTable).filter((event) => {
      if (!event.title || !event.started_at) return false;
      return (
        format(toTz(event.started_at as string, tz), "yyyy-MM-dd") === todayKey
      );
    }).length;
  }, [eventsTable, todayKey, tz]);

  const recordedThisWeekCount = useMemo(() => {
    const weekStartMs = startOfWeek(today, { weekStartsOn }).getTime();
    return sessions.filter((session) => {
      const created = new Date(session.created_at).getTime();
      return !Number.isNaN(created) && created >= weekStartMs;
    }).length;
  }, [sessions, today, weekStartsOn]);

  const recentNotes = sessions.slice(0, RECENT_NOTES_LIMIT);
  const seriesIdBySessionId = useMemo(
    () =>
      new Map(sessionLinks.map((link) => [link.session_id, link.series_id])),
    [sessionLinks],
  );

  const openSession = (sessionId: string) =>
    openCurrent({ type: "sessions", id: sessionId });

  const prepareEvent = async (
    eventId: string,
    title: string,
    _seriesId?: string,
  ) => {
    try {
      const sessionId = await getOrCreateSessionForEventId(eventId, title);
      openSession(sessionId);
    } catch (error) {
      console.error("[home] failed to prepare event", error);
    }
  };

  const eventGroups = [
    { id: "today", label: t`Today` },
    { id: "tomorrow", label: t`Tomorrow` },
    { id: "later", label: t`Later this week` },
  ];

  return (
    <div className="relative h-full w-full">
      <div className="mx-auto flex h-full w-full max-w-[700px] flex-col overflow-y-auto px-8 pt-14 pb-9">
        <div className="text-muted-foreground text-[11px] font-semibold tracking-wide uppercase">
          {format(today, "EEEE · MMMM d, yyyy")}
        </div>
        <h1 className="page-title mt-2">
          <Trans>A clear view of the day.</Trans>
        </h1>
        <div className="text-muted-foreground border-border mt-4 grid grid-cols-4 border-y py-3 text-xs">
          <button
            type="button"
            onClick={() => openSurfaceTab("calendar")}
            className="hover:text-foreground text-left"
          >
            <span className="block">
              <Trans>Scheduled today</Trans>
            </span>
            <span className="text-foreground mt-1 block font-serif text-xl tabular-nums">
              {scheduledTodayCount}
            </span>
          </button>
          <button
            type="button"
            onClick={() => openSurfaceTab("meetings")}
            className="hover:text-foreground text-left"
          >
            <span className="block">
              <Trans>Recorded this week</Trans>
            </span>
            <span className="text-foreground mt-1 block font-serif text-xl tabular-nums">
              {recordedThisWeekCount}
            </span>
          </button>
          <button
            type="button"
            onClick={() => openSurfaceTab("action-items")}
            className="hover:text-foreground text-left"
          >
            <span className="block">
              <Trans>Open actions</Trans>
            </span>
            <span className="text-foreground mt-1 block font-serif text-xl tabular-nums">
              {openActionItemsCount}
            </span>
          </button>
          <button
            type="button"
            onClick={() => openSurfaceTab("meetings")}
            className="hover:text-foreground text-left"
          >
            <span className="block">
              <Trans>Need a summary</Trans>
            </span>
            <span className="text-foreground mt-1 block font-serif text-xl tabular-nums">
              {sessionsNeedingSummaryCount}
            </span>
          </button>
        </div>

        <section className="mt-7">
          <div className="mb-2.5 flex items-center justify-between">
            <h2 className="font-serif text-[19px] font-medium">
              <Trans>Coming up</Trans>
            </h2>
            <button
              type="button"
              onClick={() => openSurfaceTab("calendar")}
              className="text-muted-foreground hover:text-foreground text-xs"
            >
              <Trans>Open calendar</Trans>
            </button>
          </div>
          {comingUp.length === 0 ? (
            <p className="text-muted-foreground text-sm">
              <Trans>Nothing on the calendar.</Trans>
            </p>
          ) : (
            <div className="divide-border divide-y">
              {eventGroups.map(({ id, label }) => {
                const events = comingUp.filter(
                  (event) => event.dayLabel === id,
                );
                if (!events.length) return null;
                return (
                  <div key={id}>
                    <h3 className="text-muted-foreground py-2 text-[11px] font-semibold tracking-wide uppercase">
                      {label}
                    </h3>
                    {events.map((event) => (
                      <div
                        key={event.id}
                        className="flex items-center gap-3 py-2.5 text-sm"
                      >
                        <span className="text-muted-foreground w-[82px] shrink-0 font-mono text-[11px] tabular-nums">
                          {format(toTz(event.startedAt, tz), "HH:mm")}
                          {event.endedAt
                            ? `–${format(toTz(event.endedAt, tz), "HH:mm")}`
                            : ""}
                        </span>
                        <div className="min-w-0 flex-1">
                          <span className="font-medium">{event.title}</span>
                          {event.isRecurring ? (
                            <span
                              className="text-muted-foreground ml-1.5 inline-flex items-center gap-1 align-text-top"
                              title={
                                event.cadence === "weekly"
                                  ? t`Weekly`
                                  : event.cadence === "biweekly"
                                    ? t`Every two weeks`
                                    : event.cadence === "monthly"
                                      ? t`Monthly`
                                      : t`Recurring event`
                              }
                            >
                              <RepeatIcon size={13} />
                            </span>
                          ) : null}
                          {event.attendees.length ? (
                            <div className="text-muted-foreground truncate text-xs">
                              {event.attendees.join(", ")}
                            </div>
                          ) : null}
                        </div>
                        {event.sessionId ? (
                          <button
                            type="button"
                            onClick={() =>
                              openSession(event.sessionId as string)
                            }
                            className="bg-foreground text-background hover:bg-foreground/90 shrink-0 rounded-full px-3 py-1.5 text-xs font-medium transition-colors"
                          >
                            <Trans>Open notes</Trans>
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() =>
                              void prepareEvent(
                                event.id,
                                event.title,
                                event.seriesId,
                              )
                            }
                            className="border-border text-foreground hover:bg-accent/50 shrink-0 rounded-full border px-3 py-1.5 text-xs font-medium transition-colors"
                          >
                            <Trans>Prepare</Trans>
                          </button>
                        )}
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <section className="mt-7">
          <div className="mb-2.5 flex items-center justify-between">
            <h2 className="font-serif text-[19px] font-medium">
              <Trans>Recent notes</Trans>
            </h2>
            <button
              type="button"
              onClick={() => openSurfaceTab("meetings")}
              className="text-muted-foreground hover:text-foreground text-xs"
            >
              <Trans>All meetings</Trans>
            </button>
          </div>
          {recentNotes.length === 0 ? (
            <p className="text-muted-foreground text-sm">
              <Trans>No notes yet.</Trans>
            </p>
          ) : (
            <div className="divide-border divide-y">
              {recentNotes.map((session) => (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => openSession(session.id)}
                  className="hover:bg-accent/50 flex w-full items-center gap-3 rounded-md py-2.5 text-left text-sm"
                >
                  <span className="flex-1 truncate font-medium">
                    {session.title || t`Untitled`}
                    {seriesIdBySessionId.get(session.id) ? (
                      <RepeatIcon
                        size={13}
                        className="text-muted-foreground ml-1.5 inline-block align-text-top"
                      />
                    ) : null}
                  </span>
                  <span className="text-muted-foreground shrink-0 text-xs">
                    {formatDistance(toTz(session.created_at, tz), today, {
                      addSuffix: true,
                    })}
                  </span>
                </button>
              ))}
            </div>
          )}
        </section>

        <div className="h-16 shrink-0" />
      </div>
      <Dock />
    </div>
  );
}

function getRecurrenceCadence(
  eventsTable: TimelineEventsTable,
  seriesId: string | null | undefined,
): RecurrenceCadence {
  if (!seriesId || !eventsTable) return "unknown";
  const dates = Object.values(eventsTable)
    .filter(
      (event) =>
        event.recurrence_series_id === seriesId && event.started_at != null,
    )
    .map((event) => new Date(event.started_at as string).getTime())
    .filter(Number.isFinite)
    .sort((a, b) => a - b);
  const gaps = dates
    .slice(1)
    .map((date, index) => Math.round((date - dates[index]) / 86_400_000))
    .filter((gap) => gap > 0);
  if (!gaps.length) return "unknown";
  const gap = Math.min(...gaps);
  if (gap >= 26) return "monthly";
  if (gap >= 12) return "biweekly";
  if (gap >= 5) return "weekly";
  return "unknown";
}
