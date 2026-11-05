import { Trans } from "@lingui/react/macro";
import { format, isValid, parseISO } from "date-fns";
import { useCallback } from "react";

import { useLiveQuery } from "~/db";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

type SeriesOccurrenceRow = {
  id: string;
  title: string | null;
  created_at: string;
};

const EMPTY_OCCURRENCES: SeriesOccurrenceRow[] = [];

/**
 * Wave 1 placeholder: the series title and its occurrences, newest first.
 * Wave 2 (slice F) owns carried-over actions and the delta-since-last-time
 * view from series.html.
 */
export function TabContentSeries({
  tab,
}: {
  tab: Extract<Tab, { type: "series" }>;
}) {
  return (
    <StandardContentWrapper>
      <SeriesView seriesId={tab.state.seriesId} />
    </StandardContentWrapper>
  );
}

function SeriesView({ seriesId }: { seriesId: string }) {
  const { data: occurrences = EMPTY_OCCURRENCES } = useLiveQuery<
    SeriesOccurrenceRow,
    SeriesOccurrenceRow[]
  >({
    sql: `
      SELECT id, title, created_at
      FROM sessions
      WHERE series_id = ? AND deleted_at IS NULL
      ORDER BY COALESCE(NULLIF(started_at, ''), created_at) DESC
    `,
    params: [seriesId],
    enabled: Boolean(seriesId),
  });

  const openCurrent = useTabs((state) => state.openCurrent);
  const openSession = useCallback(
    (sessionId: string) => {
      openCurrent({ type: "sessions", id: sessionId });
    },
    [openCurrent],
  );

  return (
    <div className="mx-auto w-full max-w-[700px] px-8 py-9">
      <h1 className="page-title">
        <Trans>Series</Trans>
      </h1>
      <p className="text-muted-foreground mt-1 text-xs">
        <Trans>{occurrences.length} occurrences</Trans>
      </p>

      <div className="divide-border border-border mt-6 divide-y border-t">
        {occurrences.map((occurrence) => {
          const created = parseISO(occurrence.created_at);
          return (
            <button
              key={occurrence.id}
              type="button"
              onClick={() => openSession(occurrence.id)}
              className="hover:bg-accent/50 flex w-full items-center justify-between gap-3 px-1 py-2.5 text-left text-sm"
            >
              <span className="truncate">
                {occurrence.title || <Trans>Untitled</Trans>}
              </span>
              <span className="text-muted-foreground shrink-0 text-xs">
                {isValid(created) ? format(created, "MMM d, yyyy") : ""}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
