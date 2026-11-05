import { Trans, useLingui } from "@lingui/react/macro";
import {
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  endOfWeek,
  format,
  isSameDay,
  isSameMonth,
  parseISO,
  startOfMonth,
  startOfWeek,
  subMonths,
} from "date-fns";
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { EventChip } from "~/calendar/components/event-chip";
import {
  toTz,
  useCalendarData,
  useNow,
  useTimezone,
  useWeekStartsOn,
} from "~/calendar/hooks";
import { useCalendarSelection } from "~/calendar/selection";
import { openSurfaceTab } from "~/sidebar/surface-navigation";

export function CalendarNav() {
  const { t } = useLingui();
  const now = useNow();
  const tz = useTimezone();
  const weekStartsOn = useWeekStartsOn();
  const calendarData = useCalendarData();
  const selectedDateValue = useCalendarSelection((state) => state.selectedDate);
  const selectDate = useCalendarSelection((state) => state.selectDate);
  const selectedDate = selectedDateValue ? parseISO(selectedDateValue) : now;
  const [visibleMonth, setVisibleMonth] = useState(startOfMonth(selectedDate));
  const monthStart = startOfWeek(startOfMonth(visibleMonth), { weekStartsOn });
  const monthEnd = endOfWeek(endOfMonth(visibleMonth), { weekStartsOn });
  useEffect(() => {
    if (selectedDateValue)
      setVisibleMonth(startOfMonth(parseISO(selectedDateValue)));
  }, [selectedDateValue]);
  const monthDays = useMemo(
    () => eachDayOfInterval({ start: monthStart, end: monthEnd }),
    [monthEnd, monthStart],
  );
  const selectedKey = format(selectedDate, "yyyy-MM-dd");
  const events = (calendarData.eventIdsByDate[selectedKey] ?? []).map((id) => ({
    id,
    event: calendarData.eventsById[id],
  }));
  const weekdayLabels =
    weekStartsOn === 1
      ? [t`Mon`, t`Tue`, t`Wed`, t`Thu`, t`Fri`, t`Sat`, t`Sun`]
      : [t`Sun`, t`Mon`, t`Tue`, t`Wed`, t`Thu`, t`Fri`, t`Sat`];

  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <CustomSidebarHeader title={<Trans>Calendar</Trans>} />
      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto px-3">
        <section aria-label={t`Mini calendar`} className="pt-3">
          <div className="mb-2 flex items-center justify-between">
            <h2 className="text-sm font-semibold">
              {format(visibleMonth, "MMMM yyyy")}
            </h2>
            <div className="flex items-center">
              <button
                type="button"
                aria-label={t`Previous month`}
                className="text-muted-foreground hover:bg-accent hover:text-foreground rounded p-1"
                onClick={() => setVisibleMonth((month) => subMonths(month, 1))}
              >
                <ChevronLeftIcon className="size-3.5" />
              </button>
              <button
                type="button"
                aria-label={t`Next month`}
                className="text-muted-foreground hover:bg-accent hover:text-foreground rounded p-1"
                onClick={() => setVisibleMonth((month) => addMonths(month, 1))}
              >
                <ChevronRightIcon className="size-3.5" />
              </button>
            </div>
          </div>
          <div className="grid grid-cols-7 text-center">
            {weekdayLabels.map((label, index) => (
              <span
                key={`${label}-${index}`}
                className="text-muted-foreground py-1 text-[10px]"
              >
                {label}
              </span>
            ))}
            {monthDays.map((day) => {
              const key = format(day, "yyyy-MM-dd");
              const hasEvents =
                (calendarData.eventIdsByDate[key]?.length ?? 0) > 0;
              const isToday = isSameDay(day, now);
              const isSelected = isSameDay(day, selectedDate);
              return (
                <button
                  key={key}
                  type="button"
                  aria-label={format(day, "EEEE, MMMM d")}
                  aria-pressed={isSelected}
                  onClick={() => {
                    selectDate(day);
                    setVisibleMonth(startOfMonth(day));
                  }}
                  className={cn(
                    "relative mx-auto flex size-7 items-center justify-center rounded-full text-xs",
                    isSelected && "bg-primary text-primary-foreground",
                    !isSelected && isToday && "ring-primary ring-1",
                    !isSelected &&
                      !isSameMonth(day, visibleMonth) &&
                      "text-muted-foreground/60",
                    !isSelected && "hover:bg-accent",
                  )}
                >
                  {format(day, "d")}
                  {hasEvents && (
                    <span
                      aria-hidden="true"
                      className={cn(
                        "absolute bottom-0.5 size-1 rounded-full",
                        isSelected ? "bg-primary-foreground" : "bg-primary",
                      )}
                    />
                  )}
                </button>
              );
            })}
          </div>
        </section>
        <section aria-label={t`Selected day events`} className="mt-4">
          <h3 className="border-border text-muted-foreground mb-1 border-b pb-2 text-[10px] font-semibold tracking-[0.08em] uppercase">
            {format(selectedDate, "EEEE, MMMM d")}
          </h3>
          {events.length === 0 ? (
            <p className="text-muted-foreground py-3 text-xs">
              <Trans>No events</Trans>
            </p>
          ) : (
            <div className="divide-border divide-y">
              {events.map(({ id, event }) => {
                const time = event?.is_all_day
                  ? t`All day`
                  : event?.started_at
                    ? format(toTz(event.started_at, tz), "HH:mm")
                    : "";
                return (
                  <div key={id} className="flex items-start gap-2 py-2">
                    <span className="text-muted-foreground w-10 shrink-0 pt-1 text-[10px] tabular-nums">
                      {time}
                    </span>
                    <div className="min-w-0 flex-1">
                      <EventChip eventId={id} event={event} showTime={false} />
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
        <button
          type="button"
          className="text-muted-foreground hover:bg-accent hover:text-foreground mt-3 w-full rounded-full px-3 py-1.5 text-left text-xs"
          onClick={() => openSurfaceTab("settings")}
        >
          <Trans>Manage calendars</Trans>
        </button>
      </div>
    </div>
  );
}
