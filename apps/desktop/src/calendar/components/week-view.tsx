import { format, isSameDay } from "date-fns";
import { useMemo } from "react";

import { cn } from "@hypr/utils";

import { EventChip } from "~/calendar/components/event-chip";
import { toTz, useNow, useTimezone } from "~/calendar/hooks";
import type { CalendarData } from "~/calendar/hooks";

const FIRST_HOUR = 8;
const LAST_HOUR = 19;
const HOUR_HEIGHT = 64;

export function WeekView({
  days,
  calendarData,
}: {
  days: Date[];
  calendarData: CalendarData;
}) {
  const now = useNow();
  const tz = useTimezone();
  const timeLabels = useMemo(
    () =>
      Array.from(
        { length: LAST_HOUR - FIRST_HOUR },
        (_, index) => FIRST_HOUR + index,
      ),
    [],
  );

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div
        className="grid min-w-[640px] grid-cols-[48px_repeat(7,minmax(0,1fr))]"
        style={{
          gridTemplateRows: `40px ${HOUR_HEIGHT * (LAST_HOUR - FIRST_HOUR)}px`,
        }}
      >
        <div className="border-border bg-background sticky top-0 z-20 border-b" />
        {days.map((day) => (
          <div
            key={`head-${day.toISOString()}`}
            className={cn(
              "border-border bg-background sticky top-0 z-20 border-b border-l px-2 py-1.5 text-center",
              isSameDay(day, now) && "bg-muted/50",
            )}
          >
            <span className="text-muted-foreground block text-[10px] uppercase">
              {format(day, "EEE")}
            </span>
            <span
              className={cn(
                "text-sm",
                isSameDay(day, now) && "text-primary font-semibold",
              )}
            >
              {format(day, "d")}
            </span>
          </div>
        ))}
        <div
          className="border-border relative border-r"
          style={{ height: HOUR_HEIGHT * (LAST_HOUR - FIRST_HOUR) }}
        >
          {timeLabels.map((hour) => (
            <div
              key={hour}
              className="text-muted-foreground absolute right-2 -translate-y-1/2 text-[11px] tabular-nums"
              style={{ top: (hour - FIRST_HOUR) * HOUR_HEIGHT }}
            >
              {format(new Date(2020, 0, 1, hour), "h a")}
            </div>
          ))}
        </div>
        {days.map((day) => {
          const dayKey = format(day, "yyyy-MM-dd");
          const eventIds = calendarData.eventIdsByDate[dayKey] ?? [];
          return (
            <div
              key={`day-${day.toISOString()}`}
              className={cn(
                "border-border relative border-r",
                isSameDay(day, now) && "bg-muted/30",
              )}
              style={{ height: HOUR_HEIGHT * (LAST_HOUR - FIRST_HOUR) }}
            >
              {timeLabels.map((hour) => (
                <div
                  key={hour}
                  aria-hidden="true"
                  className="border-border/70 absolute inset-x-0 border-t"
                  style={{ top: (hour - FIRST_HOUR) * HOUR_HEIGHT }}
                />
              ))}
              <div
                className="border-border/70 absolute inset-x-0 border-t"
                style={{ top: HOUR_HEIGHT * (LAST_HOUR - FIRST_HOUR) }}
              />
              {eventIds.map((eventId) => {
                const event = calendarData.eventsById[eventId];
                if (!event?.started_at || event.is_all_day) return null;
                const start = toTz(event.started_at, tz);
                const end = event.ended_at
                  ? toTz(event.ended_at, tz)
                  : new Date(start.getTime() + 30 * 60_000);
                const startMinutes = start.getHours() * 60 + start.getMinutes();
                const endMinutes = Math.max(
                  startMinutes + 15,
                  end.getHours() * 60 + end.getMinutes(),
                );
                const visibleStart = FIRST_HOUR * 60;
                const visibleEnd = LAST_HOUR * 60;
                if (endMinutes <= visibleStart || startMinutes >= visibleEnd)
                  return null;
                const top =
                  ((Math.max(startMinutes, visibleStart) - visibleStart) / 60) *
                  HOUR_HEIGHT;
                const height = Math.max(
                  24,
                  ((Math.min(endMinutes, visibleEnd) -
                    Math.max(startMinutes, visibleStart)) /
                    60) *
                    HOUR_HEIGHT,
                );
                return (
                  <div
                    key={eventId}
                    className="absolute z-10 px-1"
                    style={{ top, height, left: 0, right: 0 }}
                  >
                    <EventChip
                      eventId={eventId}
                      event={event}
                      className="border-border bg-card h-full items-start rounded-md border border-l-2 px-1.5 py-1 shadow-none"
                    />
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
