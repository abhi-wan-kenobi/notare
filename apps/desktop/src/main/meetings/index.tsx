import { Trans } from "@lingui/react/macro";
import { format, isValid, parseISO } from "date-fns";
import { useCallback, useMemo } from "react";

import { StandardContentWrapper } from "~/shared/main";
import { useSessionSummaries } from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs";

/**
 * Wave 1 placeholder: a plain, data-backed list of sessions grouped by day.
 * Wave 2 (slice E) owns the real filters/search/grouping from meetings.html.
 */
export function TabContentMeetings() {
  return (
    <StandardContentWrapper>
      <MeetingsView />
    </StandardContentWrapper>
  );
}

function MeetingsView() {
  const sessions = useSessionSummaries();
  const openCurrent = useTabs((state) => state.openCurrent);

  const openSession = useCallback(
    (sessionId: string) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    [openCurrent],
  );

  const groups = useMemo(() => {
    const byDay = new Map<string, typeof sessions>();
    for (const session of sessions) {
      const created = parseISO(session.created_at);
      const key = isValid(created)
        ? format(created, "EEEE, MMMM d")
        : "Unknown";
      const existing = byDay.get(key);
      if (existing) {
        existing.push(session);
      } else {
        byDay.set(key, [session]);
      }
    }
    return Array.from(byDay.entries());
  }, [sessions]);

  return (
    <div className="mx-auto w-full max-w-[700px] px-8 py-9">
      <h1 className="page-title">
        <Trans>Meetings</Trans>
      </h1>

      {groups.length === 0 ? (
        <p className="text-muted-foreground mt-6 text-sm">
          <Trans>No meetings yet.</Trans>
        </p>
      ) : (
        groups.map(([day, daySessions]) => (
          <section key={day} className="mt-7">
            <h2 className="text-muted-foreground font-serif text-[15px] font-medium">
              {day}
            </h2>
            <div className="border-border/80 mt-1 divide-y divide-border border-t">
              {daySessions.map((session) => (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => openSession(session.id)}
                  className="hover:bg-accent/50 flex w-full items-center gap-3 px-1 py-2.5 text-left text-sm"
                >
                  <span className="flex-1 truncate">
                    {session.title || <Trans>Untitled</Trans>}
                  </span>
                </button>
              ))}
            </div>
          </section>
        ))
      )}
    </div>
  );
}
