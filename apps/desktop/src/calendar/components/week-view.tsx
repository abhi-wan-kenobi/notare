import { useLingui } from "@lingui/react/macro";
import { format, isSameDay } from "date-fns";
import { RepeatIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";

import { cn } from "@hypr/utils";

import { toTz, useNow, useTimezone } from "~/calendar/hooks";
import type { CalendarData } from "~/calendar/hooks";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import { getOrCreateSessionForEventId } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";
import { useTabs } from "~/store/zustand/tabs";

const DEFAULT_FIRST_HOUR = 7;
const DEFAULT_LAST_HOUR = 21;
const HOUR_HEIGHT = 64;

type TimedEventLayout = {
  eventId: string;
  event: TimelineEventRow;
  start: Date;
  end: Date;
  startMinutes: number;
  endMinutes: number;
  top: number;
  height: number;
  left: string;
  width: string;
};

export function WeekView({
  days,
  calendarData,
}: {
  days: Date[];
  calendarData: CalendarData;
}) {
  const { t } = useLingui();
  const now = useNow();
  const tz = useTimezone();
  const scrollRef = useRef<HTMLDivElement>(null);

  const { firstHour, lastHour } = useMemo(() => {
    let earliest = DEFAULT_FIRST_HOUR;
    let latest = DEFAULT_LAST_HOUR;
    for (const day of days) {
      const dayKey = format(day, "yyyy-MM-dd");
      const eventIds = calendarData.eventIdsByDate[dayKey] ?? [];
      for (const id of eventIds) {
        const ev = calendarData.eventsById[id];
        if (ev?.started_at && !ev.is_all_day) {
          const h = toTz(ev.started_at, tz).getHours();
          if (h < earliest) earliest = Math.max(0, h);
          const endH = ev.ended_at
            ? toTz(ev.ended_at, tz).getHours() + 1
            : h + 1;
          if (endH > latest) latest = Math.min(24, endH);
        }
      }
    }
    return { firstHour: earliest, lastHour: latest };
  }, [days, calendarData, tz]);

  const timeLabels = useMemo(
    () =>
      Array.from(
        { length: lastHour - firstHour },
        (_, index) => firstHour + index,
      ),
    [firstHour, lastHour],
  );

  const hasAllDayEvents = useMemo(() => {
    return days.some((day) => {
      const dayKey = format(day, "yyyy-MM-dd");
      const eventIds = calendarData.eventIdsByDate[dayKey] ?? [];
      return eventIds.some((id) => calendarData.eventsById[id]?.is_all_day);
    });
  }, [days, calendarData]);

  // Scroll to roughly 1 hour before current time on initial mount if today is visible
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const isTodayInView = days.some((d) => isSameDay(d, now));
    if (isTodayInView) {
      const targetHour = Math.max(firstHour, now.getHours() - 1);
      const scrollPos = (targetHour - firstHour) * HOUR_HEIGHT;
      el.scrollTo?.({ top: scrollPos, behavior: "instant" });
    }
  }, [days, firstHour, now]);

  return (
    <div
      ref={scrollRef}
      className="scrollbar-hide border-border/60 min-h-0 flex-1 overflow-auto border-t"
    >
      <div
        className="grid min-w-[700px] grid-cols-[56px_repeat(7,minmax(0,1fr))]"
        style={{
          gridTemplateRows: `${
            hasAllDayEvents ? "auto auto" : "auto"
          } ${HOUR_HEIGHT * (lastHour - firstHour)}px`,
        }}
      >
        {/* Header row: time gutter corner */}
        <div className="border-border/60 bg-background sticky top-0 z-20 border-r border-b" />

        {/* Header row: day names and numbers */}
        {days.map((day) => {
          const isToday = isSameDay(day, now);
          return (
            <div
              key={`head-${day.toISOString()}`}
              className={cn(
                "border-border/60 bg-background sticky top-0 z-20 border-r border-b px-2 py-2 text-center select-none",
                isToday && "bg-muted/40",
              )}
            >
              <span className="text-muted-foreground block text-[10px] font-semibold tracking-wider uppercase">
                {format(day, "EEE")}
              </span>
              <span
                className={cn(
                  "mt-0.5 inline-flex size-6 items-center justify-center rounded-full text-sm font-medium",
                  isToday && "bg-primary text-primary-foreground font-semibold",
                )}
              >
                {format(day, "d")}
              </span>
            </div>
          );
        })}

        {/* Optional All-Day Events row */}
        {hasAllDayEvents && (
          <>
            <div className="border-border/60 bg-background text-muted-foreground sticky top-[53px] z-10 flex items-center justify-end border-r border-b pr-2 text-[10px] font-medium tracking-wide uppercase">
              {t`All day`}
            </div>
            {days.map((day) => {
              const dayKey = format(day, "yyyy-MM-dd");
              const eventIds = calendarData.eventIdsByDate[dayKey] ?? [];
              const allDayEvents = eventIds
                .map((id) => ({ id, event: calendarData.eventsById[id] }))
                .filter(
                  (item): item is { id: string; event: TimelineEventRow } =>
                    !!item.event?.is_all_day,
                );
              const isToday = isSameDay(day, now);

              return (
                <div
                  key={`allday-${day.toISOString()}`}
                  className={cn(
                    "border-border/60 bg-background sticky top-[53px] z-10 space-y-1 border-r border-b p-1",
                    isToday && "bg-muted/40",
                  )}
                >
                  {allDayEvents.map(({ id, event }) => (
                    <AllDayEventChip
                      key={id}
                      eventId={id}
                      event={event}
                    />
                  ))}
                </div>
              );
            })}
          </>
        )}

        {/* Time gutter labels */}
        <div
          className="border-border/60 relative select-none border-r"
          style={{ height: HOUR_HEIGHT * (lastHour - firstHour) }}
        >
          {timeLabels.map((hour) => (
            <div
              key={hour}
              className="text-muted-foreground absolute inset-x-0 -translate-y-1/2 pr-2 text-right font-mono text-[13px] tabular-nums"
              style={{ top: (hour - firstHour) * HOUR_HEIGHT }}
            >
              {format(new Date(2020, 0, 1, hour), "h a")}
            </div>
          ))}
        </div>

        {/* Day columns with hairline grid and quiet event blocks */}
        {days.map((day) => {
          const isToday = isSameDay(day, now);
          const dayKey = format(day, "yyyy-MM-dd");
          const eventIds = calendarData.eventIdsByDate[dayKey] ?? [];

          const timedLayouts = computeTimedEventLayouts({
            eventIds,
            eventsById: calendarData.eventsById,
            tz,
            firstHour,
            lastHour,
          });

          // Current time position for today
          const nowMinutes = now.getHours() * 60 + now.getMinutes();
          const showCurrentTime =
            isToday &&
            nowMinutes >= firstHour * 60 &&
            nowMinutes <= lastHour * 60;
          const nowTop = ((nowMinutes - firstHour * 60) / 60) * HOUR_HEIGHT;

          return (
            <div
              key={`day-${day.toISOString()}`}
              className={cn(
                "border-border/60 relative border-r",
                isToday && "bg-muted/30",
              )}
              style={{ height: HOUR_HEIGHT * (lastHour - firstHour) }}
            >
              {/* Horizontal hairline grid lines */}
              {timeLabels.map((hour) => (
                <div
                  key={hour}
                  aria-hidden="true"
                  className="border-border/60 absolute inset-x-0 border-t"
                  style={{ top: (hour - firstHour) * HOUR_HEIGHT }}
                />
              ))}
              <div
                aria-hidden="true"
                className="border-border/60 absolute inset-x-0 border-t"
                style={{ top: HOUR_HEIGHT * (lastHour - firstHour) }}
              />

              {/* Today's current time indicator */}
              {showCurrentTime && (
                <div
                  aria-hidden="true"
                  className="pointer-events-none absolute inset-x-0 z-20 flex items-center"
                  style={{ top: nowTop }}
                >
                  <div className="bg-destructive -ml-1 size-2 rounded-full" />
                  <div className="bg-destructive/80 h-0.5 w-full" />
                </div>
              )}

              {/* Quiet rounded event blocks */}
              {timedLayouts.map((item) => (
                <WeekEventBlock
                  key={item.eventId}
                  layout={item}
                />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function computeTimedEventLayouts({
  eventIds,
  eventsById,
  tz,
  firstHour,
  lastHour,
}: {
  eventIds: string[];
  eventsById: Record<string, TimelineEventRow | undefined>;
  tz: string | undefined;
  firstHour: number;
  lastHour: number;
}): TimedEventLayout[] {
  const visibleStart = firstHour * 60;
  const visibleEnd = lastHour * 60;

  const valid: Array<{
    eventId: string;
    event: TimelineEventRow;
    start: Date;
    end: Date;
    startMinutes: number;
    endMinutes: number;
  }> = [];

  for (const id of eventIds) {
    const ev = eventsById[id];
    if (!ev?.started_at || ev.is_all_day) continue;
    const start = toTz(ev.started_at, tz);
    const end = ev.ended_at
      ? toTz(ev.ended_at, tz)
      : new Date(start.getTime() + 30 * 60_000);
    const startMinutes = start.getHours() * 60 + start.getMinutes();
    const endMinutes = Math.max(
      startMinutes + 15,
      end.getHours() * 60 + end.getMinutes(),
    );

    if (endMinutes <= visibleStart || startMinutes >= visibleEnd) continue;
    valid.push({
      eventId: id,
      event: ev,
      start,
      end,
      startMinutes,
      endMinutes,
    });
  }

  // Sort by start time, longer duration first
  valid.sort((a, b) => {
    if (a.startMinutes !== b.startMinutes) return a.startMinutes - b.startMinutes;
    return (b.endMinutes - b.startMinutes) - (a.endMinutes - a.startMinutes);
  });

  // Calculate overlapping groups to allocate columns
  const clusters: Array<typeof valid> = [];
  let currentCluster: typeof valid = [];
  let clusterEnd = -1;

  for (const item of valid) {
    if (currentCluster.length === 0) {
      currentCluster.push(item);
      clusterEnd = item.endMinutes;
    } else if (item.startMinutes < clusterEnd) {
      currentCluster.push(item);
      clusterEnd = Math.max(clusterEnd, item.endMinutes);
    } else {
      clusters.push(currentCluster);
      currentCluster = [item];
      clusterEnd = item.endMinutes;
    }
  }
  if (currentCluster.length > 0) {
    clusters.push(currentCluster);
  }

  const results: TimedEventLayout[] = [];

  for (const cluster of clusters) {
    const totalCols = Math.min(cluster.length, 3);
    const columns: number[] = [];

    cluster.forEach((item, index) => {
      // Find first column that is free
      let colIdx = columns.findIndex((colEnd) => colEnd <= item.startMinutes);
      if (colIdx === -1) {
        colIdx = columns.length < totalCols ? columns.length : index % totalCols;
        columns[colIdx] = item.endMinutes;
      } else {
        columns[colIdx] = item.endMinutes;
      }

      const top =
        ((Math.max(item.startMinutes, visibleStart) - visibleStart) / 60) *
        HOUR_HEIGHT;
      const height = Math.max(
        26,
        ((Math.min(item.endMinutes, visibleEnd) -
          Math.max(item.startMinutes, visibleStart)) /
          60) *
          HOUR_HEIGHT -
          2,
      );

      const colCount = Math.max(1, Math.min(cluster.length, totalCols));
      const width =
        colCount === 1
          ? "calc(100% - 8px)"
          : `calc(${100 / colCount}% - 4px)`;
      const left =
        colCount === 1
          ? "4px"
          : `calc(${colIdx * (100 / colCount)}% + 2px)`;

      results.push({
        ...item,
        top,
        height,
        left,
        width,
      });
    });
  }

  return results;
}

function WeekEventBlock({ layout }: { layout: TimedEventLayout }) {
  const { t } = useLingui();
  const openCurrent = useTabs((state) => state.openCurrent);
  const { ignoreEvent, ignoreSeries } = useIgnoredEvents();
  const { eventId, event, start, end, top, height, left, width } = layout;

  const title = event.title || t`Untitled`;
  const color = event.calendar_color || "var(--color-primary)";
  const durationMinutes = Math.max(
    15,
    Math.round((end.getTime() - start.getTime()) / 60_000),
  );

  const handleOpen = useCallback(() => {
    void getOrCreateSessionForEventId(eventId, title)
      .then((sessionId) => {
        openCurrent({ type: "sessions", id: sessionId });
      })
      .catch((error) => {
        console.error("[calendar] failed to open event note", error);
      });
  }, [eventId, title, openCurrent]);

  const handleIgnore = useCallback(() => {
    if (event.tracking_id_event) {
      ignoreEvent(event.tracking_id_event);
    }
  }, [event.tracking_id_event, ignoreEvent]);

  const handleIgnoreSeries = useCallback(() => {
    if (event.recurrence_series_id) {
      ignoreSeries(event.recurrence_series_id);
    }
  }, [event.recurrence_series_id, ignoreSeries]);

  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const items: MenuItemDef[] = [
      {
        id: "open",
        text: t`Open note`,
        action: handleOpen,
      },
      {
        id: "ignore",
        text: event.recurrence_series_id
          ? t`Delete This Event`
          : t`Delete Event`,
        action: handleIgnore,
      },
    ];
    if (event.recurrence_series_id) {
      items.push({
        id: "ignore-series",
        text: t`Delete Entire Series`,
        action: handleIgnoreSeries,
      });
    }
    return items;
  }, [event.recurrence_series_id, handleOpen, handleIgnore, handleIgnoreSeries, t]);

  const showContextMenu = useNativeContextMenu(contextMenu);

  const isShort = height < 44;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          handleOpen();
        }
      }}
      onContextMenu={showContextMenu}
      className={cn(
        "group absolute z-10 flex flex-col justify-start rounded-[7px] border border-l-2 p-1.5 text-left transition-colors select-none",
        "bg-card border-border/70 hover:bg-accent/40 hover:border-border cursor-pointer",
        "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-1",
      )}
      style={{
        top,
        height,
        left,
        width,
        borderLeftColor: color,
      }}
      title={`${title} (${format(start, "h:mm a")} - ${format(end, "h:mm a")})`}
    >
      <div className="flex min-w-0 items-center gap-1">
        <strong className="text-foreground truncate text-xs font-semibold leading-tight">
          {title}
        </strong>
        {event.recurrence_series_id && (
          <span
            title={t`Recurring event`}
            className="text-muted-foreground shrink-0"
          >
            <RepeatIcon className="size-3" />
          </span>
        )}
      </div>

      {!isShort && (
        <small className="text-muted-foreground mt-0.5 block truncate font-mono text-[11px] leading-tight">
          {format(start, "h:mm")} · {durationMinutes} min
        </small>
      )}
    </div>
  );
}

function AllDayEventChip({
  eventId,
  event,
}: {
  eventId: string;
  event: TimelineEventRow;
}) {
  const { t } = useLingui();
  const openCurrent = useTabs((state) => state.openCurrent);
  const title = event.title || t`Untitled`;
  const color = event.calendar_color || "var(--color-primary)";

  const handleOpen = useCallback(() => {
    void getOrCreateSessionForEventId(eventId, title)
      .then((sessionId) => {
        openCurrent({ type: "sessions", id: sessionId });
      })
      .catch((error) => {
        console.error("[calendar] failed to open event note", error);
      });
  }, [eventId, title, openCurrent]);

  return (
    <button
      type="button"
      onClick={handleOpen}
      style={{ borderLeftColor: color }}
      className={cn(
        "bg-card border-border/70 hover:bg-accent/40 flex w-full items-center gap-1 rounded-[5px] border border-l-2 px-1.5 py-0.5 text-left text-xs transition-colors",
        "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-1",
      )}
    >
      <span className="text-foreground min-w-0 flex-1 truncate font-medium">
        {title}
      </span>
      {event.recurrence_series_id && (
        <span
          title={t`Recurring event`}
          className="text-muted-foreground shrink-0"
        >
          <RepeatIcon className="size-2.5" />
        </span>
      )}
    </button>
  );
}
