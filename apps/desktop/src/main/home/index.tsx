import { Trans, useLingui } from "@lingui/react/macro";
import { format, startOfWeek } from "date-fns";
import { RepeatIcon } from "lucide-react";
import { useMemo } from "react";

import { toTz, useTimezone, useWeekStartsOn } from "~/calendar/hooks";
import { useTimelineEventsTable } from "~/calendar/queries";
import { useLiveQuery } from "~/db";
import { useSessionSummaries } from "~/session/queries";
import { Dock } from "~/shared/dock";
import { StandardContentWrapper } from "~/shared/main";
import { useNewNote } from "~/shared/useNewNote";
import { openSurfaceTab } from "~/sidebar/surface-navigation";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const COMING_UP_LIMIT = 4;
const RECENT_NOTES_LIMIT = 5;

type SessionLink = {
  session_id: string;
  event_id: string;
  external_event_id: string;
};
const EMPTY_SESSION_LINKS: SessionLink[] = [];

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

function HomeView() {
  const { t } = useLingui();
  const tz = useTimezone();
  const now = useMemo(() => new Date(), []);
  const nowIso = useMemo(() => now.toISOString(), [now]);
  const todayKey = format(now, "yyyy-MM-dd");

  const eventsTable = useTimelineEventsTable();
  const sessions = useSessionSummaries();
  const openCurrent = useTabs((state) => state.openCurrent);
  const openNewNote = useNewNote({ behavior: "new" });

  const { data: sessionLinks = EMPTY_SESSION_LINKS } = useLiveQuery<
    SessionLink,
    SessionLink[]
  >({
    sql: `
      SELECT id AS session_id, event_id, external_event_id
      FROM sessions
      WHERE deleted_at IS NULL
        AND ((event_id IS NOT NULL AND event_id != '')
          OR (external_event_id IS NOT NULL AND external_event_id != ''))
    `,
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

  const comingUp = useMemo(() => {
    if (!eventsTable) return [];
    return Object.entries(eventsTable)
      .filter(([, event]) => event.title && event.started_at)
      .filter(([, event]) => (event.started_at as string) >= nowIso)
      .sort((a, b) =>
        String(a[1].started_at).localeCompare(String(b[1].started_at)),
      )
      .slice(0, COMING_UP_LIMIT)
      .map(([id, event]) => ({
        id,
        title: event.title as string,
        startedAt: event.started_at as string,
        isRecurring: Boolean(event.recurrence_series_id),
        sessionId:
          sessionIdByEventId.byEventId.get(id) ??
          (event.tracking_id_event
            ? sessionIdByEventId.byTrackingId.get(
                event.tracking_id_event as string,
              )
            : undefined),
      }));
  }, [eventsTable, nowIso, sessionIdByEventId]);

  const scheduledTodayCount = useMemo(() => {
    if (!eventsTable) return 0;
    return Object.values(eventsTable).filter((event) => {
      if (!event.title || !event.started_at) return false;
      return format(new Date(event.started_at), "yyyy-MM-dd") === todayKey;
    }).length;
  }, [eventsTable, todayKey]);

  const weekStartsOn = useWeekStartsOn();
  const recordedThisWeekCount = useMemo(() => {
    const weekStartMs = startOfWeek(now, { weekStartsOn }).getTime();
    return sessions.filter((session) => {
      const created = new Date(session.created_at).getTime();
      return !Number.isNaN(created) && created >= weekStartMs;
    }).length;
  }, [sessions, weekStartsOn, now]);

  const recentNotes = sessions.slice(0, RECENT_NOTES_LIMIT);

  const openSession = (sessionId: string) =>
    openCurrent({ type: "sessions", id: sessionId });

  return (
    <div className="relative h-full w-full">
      <div className="mx-auto flex h-full w-full max-w-[700px] flex-col overflow-y-auto px-8 py-9">
        <div className="text-muted-foreground text-[11px] font-semibold tracking-wide uppercase">
          {format(now, "EEEE · MMMM d, yyyy")}
        </div>
        <h1 className="page-title mt-2">
          <Trans>A clear view of the day.</Trans>
        </h1>
        <p className="text-muted-foreground mt-1 text-[13px]">
          <Trans>
            {scheduledTodayCount} scheduled today · {recordedThisWeekCount}{" "}
            recorded this week · {openActionItemsCount} open actions
          </Trans>
        </p>

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
              {comingUp.map((event) => (
                <div
                  key={event.id}
                  className="flex items-center gap-3 py-2.5 text-sm"
                >
                  <span className="text-muted-foreground w-11 shrink-0 font-mono text-[11px]">
                    {format(toTz(event.startedAt, tz), "HH:mm")}
                  </span>
                  <div className="min-w-0 flex-1">
                    <span className="font-medium">{event.title}</span>
                    {event.isRecurring ? (
                      <RepeatIcon
                        size={13}
                        className="text-muted-foreground ml-1.5 inline-block align-text-top"
                      />
                    ) : null}
                  </div>
                  {event.sessionId ? (
                    <button
                      type="button"
                      onClick={() => openSession(event.sessionId as string)}
                      className="bg-foreground text-background shrink-0 rounded-full px-3 py-1 text-xs"
                    >
                      <Trans>Open notes</Trans>
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={() => openNewNote()}
                      className="border-border shrink-0 rounded-full border px-3 py-1 text-xs"
                    >
                      <Trans>Prepare</Trans>
                    </button>
                  )}
                </div>
              ))}
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
                  </span>
                  <span className="text-muted-foreground shrink-0 text-xs">
                    {format(new Date(session.created_at), "MMM d")}
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
