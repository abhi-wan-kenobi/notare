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
import { ChevronLeftIcon, ChevronRightIcon, RepeatIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import {
  toTz,
  useCalendarData,
  useNow,
  useTimezone,
  useWeekStartsOn,
} from "~/calendar/hooks";
import { useIgnoredEvents } from "~/calendar/ignored-events";
import { useCalendarSelection } from "~/calendar/selection";
import { getOrCreateSessionForEventId } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";
import { type SettingsTab, useTabs } from "~/store/zustand/tabs";

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
            <div className="space-y-1">
              {events.map(({ id, event }) => (
                <SidebarEventRow key={id} id={id} event={event} tz={tz} />
              ))}
            </div>
          )}
        </section>
        <button
          type="button"
          className="text-muted-foreground hover:bg-accent hover:text-foreground mt-3 flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-xs transition-colors"
          onClick={() => {
            useTabs.getState().openNew({
              type: "settings",
              state: { tab: "integrations" as unknown as SettingsTab },
            });
          }}
        >
          <Trans>Manage calendars</Trans>
        </button>
      </div>
    </div>
  );
}

function SidebarEventRow({
  id,
  event,
  tz,
}: {
  id: string;
  event: TimelineEventRow | undefined;
  tz: string | undefined;
}) {
  const { t } = useLingui();
  const openCurrent = useTabs((state) => state.openCurrent);
  const { ignoreEvent, ignoreSeries } = useIgnoredEvents();

  const handleOpen = useCallback(() => {
    void getOrCreateSessionForEventId(id, event?.title || "Untitled")
      .then((sessionId) => {
        openCurrent({ type: "sessions", id: sessionId });
      })
      .catch((error) => {
        console.error("[calendar] failed to open event note", error);
      });
  }, [id, event?.title, openCurrent]);

  const handleIgnore = useCallback(() => {
    if (event?.tracking_id_event) {
      ignoreEvent(event.tracking_id_event);
    }
  }, [event?.tracking_id_event, ignoreEvent]);

  const handleIgnoreSeries = useCallback(() => {
    if (event?.recurrence_series_id) {
      ignoreSeries(event.recurrence_series_id);
    }
  }, [event?.recurrence_series_id, ignoreSeries]);

  const contextMenu = useMemo<MenuItemDef[]>(() => {
    if (!event) return [];
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
  }, [event, handleOpen, handleIgnore, handleIgnoreSeries, t]);

  const showContextMenu = useNativeContextMenu(contextMenu);

  const time = event?.is_all_day
    ? t`All day`
    : event?.started_at
      ? format(toTz(event.started_at, tz), "HH:mm")
      : "";
  const color = event?.calendar_color || "var(--color-primary)";

  return (
    <button
      type="button"
      onClick={handleOpen}
      onContextMenu={showContextMenu}
      className={cn(
        "group hover:bg-accent/60 flex w-full items-baseline gap-2 rounded-md px-1.5 py-1.5 text-left text-xs transition-colors",
        "focus-visible:ring-ring focus-visible:ring-1 focus-visible:outline-none",
      )}
    >
      <span className="text-muted-foreground w-10 shrink-0 font-mono text-[11px] font-medium tabular-nums">
        {time}
      </span>
      <div
        className="size-1.5 shrink-0 self-center rounded-full"
        style={{ backgroundColor: color }}
        aria-hidden="true"
      />
      <span className="text-foreground min-w-0 flex-1 truncate font-medium">
        {event?.title || t`Untitled`}
      </span>
      {event?.recurrence_series_id && (
        <span
          title={t`Recurring event`}
          className="text-muted-foreground shrink-0"
        >
          <RepeatIcon className="size-3" />
        </span>
      )}
    </button>
  );
}

export { CalendarNav as CalendarSidebar };
