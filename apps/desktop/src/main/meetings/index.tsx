import { Trans, useLingui } from "@lingui/react/macro";
import { cn } from "@hypr/utils";
import { format, isValid, parseISO } from "date-fns";
import { RepeatIcon, SearchIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { useLiveQuery } from "~/db";
import { StandardContentWrapper } from "~/shared/main";
import { useTabs } from "~/store/zustand/tabs";

type MeetingRow = {
  id: string;
  title: string;
  created_at: string;
  started_at: string;
  ended_at: string;
  event_json: string;
  series_id: string;
  transcript_count: number;
  action_count: number;
  attendee_count: number;
  attendees: string;
};

export type Filter = "all" | "recorded" | "upcoming" | "series";

const EMPTY: MeetingRow[] = [];
const FILTERS: Filter[] = ["all", "recorded", "upcoming", "series"];

function meetingDate(meeting: MeetingRow): Date {
  if (meeting.started_at) {
    const parsed = parseISO(meeting.started_at);
    if (isValid(parsed)) return parsed;
  }
  if (meeting.event_json) {
    try {
      const event = JSON.parse(meeting.event_json);
      if (event.started_at) {
        const parsed = parseISO(event.started_at);
        if (isValid(parsed)) return parsed;
      }
    } catch {
      // ignore malformed JSON
    }
  }
  if (meeting.created_at) {
    const parsed = parseISO(meeting.created_at);
    if (isValid(parsed)) return parsed;
  }
  return new Date(Number.NaN);
}

function isUpcoming(meeting: MeetingRow): boolean {
  const date = meetingDate(meeting);
  return isValid(date) && date.getTime() > Date.now();
}

export function TabContentMeetings() {
  return (
    <StandardContentWrapper noBorder>
      <MeetingsView />
    </StandardContentWrapper>
  );
}

function getMeetingsTabFilter(tab: unknown): Filter | undefined {
  if (!tab || typeof tab !== "object") return undefined;
  if (!("type" in tab) || tab.type !== "meetings") return undefined;
  if (!("state" in tab) || !tab.state || typeof tab.state !== "object") return undefined;
  if ("filter" in tab.state && typeof tab.state.filter === "string") {
    const f = tab.state.filter;
    if (f === "all" || f === "recorded" || f === "upcoming" || f === "series") {
      return f;
    }
  }
  return undefined;
}

function MeetingsView() {
  const { t } = useLingui();
  const currentTab = useTabs((state) => state.currentTab);
  const tabFilter = getMeetingsTabFilter(currentTab);

  const [filter, setFilter] = useState<Filter>(tabFilter ?? "all");
  const [search, setSearch] = useState("");
  const openCurrent = useTabs((state) => state.openCurrent);

  useEffect(() => {
    if (tabFilter && tabFilter !== filter) {
      setFilter(tabFilter);
    }
  }, [tabFilter]);

  const handleFilterChange = (nextFilter: Filter) => {
    setFilter(nextFilter);
    useTabs.setState((state) => {
      if (!state.currentTab || state.currentTab.type !== "meetings") return state;
      const currentState =
        "state" in state.currentTab &&
        state.currentTab.state &&
        typeof state.currentTab.state === "object"
          ? state.currentTab.state
          : {};
      const updatedTab = {
        ...state.currentTab,
        state: {
          ...currentState,
          filter: nextFilter,
        },
      };
      return {
        currentTab: updatedTab,
        tabs: state.tabs.map((t) => (t === state.currentTab ? updatedTab : t)),
      };
    });
  };

  const { data: meetings = EMPTY } = useLiveQuery<MeetingRow, MeetingRow[]>({
    sql: `
      SELECT
        s.id,
        s.title,
        s.created_at,
        s.started_at,
        s.ended_at,
        s.event_json,
        s.series_id,
        (SELECT COUNT(*) FROM transcripts t WHERE t.session_id = s.id AND t.deleted_at IS NULL) AS transcript_count,
        (SELECT COUNT(*) FROM action_items ai WHERE ai.session_id = s.id AND ai.status != 'done' AND ai.deleted_at IS NULL) AS action_count,
        (SELECT COUNT(*) FROM session_participants sp WHERE sp.session_id = s.id AND sp.deleted_at IS NULL) AS attendee_count,
        (SELECT GROUP_CONCAT(COALESCE(NULLIF(sp.display_name, ''), h.name), ', ')
          FROM session_participants sp
          LEFT JOIN humans h ON h.id = sp.human_id AND h.deleted_at IS NULL
          WHERE sp.session_id = s.id AND sp.deleted_at IS NULL) AS attendees
      FROM sessions s
      WHERE s.deleted_at IS NULL
      ORDER BY CASE WHEN s.started_at = '' THEN s.created_at ELSE s.started_at END DESC
    `,
  });

  const visible = useMemo(() => {
    const q = search.trim().toLowerCase();
    return meetings.filter((meeting) => {
      const upcoming = isUpcoming(meeting);
      const recorded = meeting.transcript_count > 0;
      const series = Boolean(meeting.series_id);

      const matchesFilter =
        filter === "all" ||
        (filter === "upcoming" && upcoming) ||
        (filter === "recorded" && recorded && !upcoming) ||
        (filter === "series" && series);

      if (!matchesFilter) return false;

      if (!q) return true;

      const searchable = `${meeting.title} ${meeting.attendees ?? ""}`.toLowerCase();
      return searchable.includes(q);
    });
  }, [meetings, filter, search]);

  const groups = useMemo(() => {
    const result = new Map<
      string,
      {
        key: string;
        labelType: "today" | "yesterday" | "last_week" | "month" | "older";
        date: Date;
        rows: MeetingRow[];
      }
    >();

    const now = new Date();
    const todayStart = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
    ).getTime();

    for (const meeting of visible) {
      const date = meetingDate(meeting);
      if (!isValid(date)) {
        const existing = result.get("older") ?? {
          key: "older",
          labelType: "older",
          date: new Date(0),
          rows: [],
        };
        existing.rows.push(meeting);
        result.set("older", existing);
        continue;
      }

      const meetingStart = new Date(
        date.getFullYear(),
        date.getMonth(),
        date.getDate(),
      ).getTime();
      const dayDelta = Math.floor((todayStart - meetingStart) / 86400000);

      let groupKey: string;
      let labelType: "today" | "yesterday" | "last_week" | "month" | "older";

      if (dayDelta === 0) {
        groupKey = "today";
        labelType = "today";
      } else if (dayDelta === 1) {
        groupKey = "yesterday";
        labelType = "yesterday";
      } else if (dayDelta > 1 && dayDelta < 8) {
        groupKey = "last_week";
        labelType = "last_week";
      } else {
        groupKey = format(date, "yyyy-MM");
        labelType = "month";
      }

      const existing = result.get(groupKey) ?? {
        key: groupKey,
        labelType,
        date,
        rows: [],
      };
      existing.rows.push(meeting);
      result.set(groupKey, existing);
    }

    return [...result.values()];
  }, [visible]);

  const renderGroupLabel = (group: {
    labelType: "today" | "yesterday" | "last_week" | "month" | "older";
    date: Date;
  }) => {
    switch (group.labelType) {
      case "today":
        return <Trans>Today</Trans>;
      case "yesterday":
        return <Trans>Yesterday</Trans>;
      case "last_week":
        return <Trans>Last week</Trans>;
      case "month":
        return format(group.date, "MMMM");
      default:
        return <Trans>Older</Trans>;
    }
  };

  return (
    <div className="mx-auto w-full max-w-[850px] px-8 pt-9 pb-14">
      <h1 className="page-title">
        <Trans>Meetings</Trans>
      </h1>
      <p className="text-muted-foreground mt-1 text-sm">
        <Trans>Your conversations, notes, and what comes next.</Trans>
      </p>

      <div className="my-6 flex flex-wrap items-center gap-3.5">
        <div
          className="flex items-center gap-1 p-0.5 rounded-[9px] border border-border/50 bg-muted/60"
          role="group"
          aria-label={t`Filter meetings`}
        >
          {FILTERS.map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={filter === value}
              onClick={() => handleFilterChange(value)}
              className={cn(
                "h-[30px] px-3 rounded-[6px] text-xs transition-colors font-medium capitalize",
                filter === value
                  ? "bg-card text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {value === "all" ? (
                <Trans>All</Trans>
              ) : value === "recorded" ? (
                <Trans>Recorded</Trans>
              ) : value === "upcoming" ? (
                <Trans>Upcoming</Trans>
              ) : (
                <Trans>Series</Trans>
              )}
            </button>
          ))}
        </div>

        <label className="flex items-center gap-2 h-9 min-w-[225px] flex-1 px-2.5 rounded-lg border border-border bg-card text-xs text-muted-foreground focus-within:ring-1 focus-within:ring-ring cursor-text">
          <SearchIcon size={14} className="shrink-0 text-muted-foreground" />
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={t`Search meetings`}
            aria-label={t`Search meetings`}
            className="w-full bg-transparent border-0 outline-none text-foreground placeholder:text-muted-foreground text-xs"
          />
        </label>
      </div>

      {groups.length === 0 ? (
        <p className="text-muted-foreground mt-6 text-sm">
          <Trans>No meetings found.</Trans>
        </p>
      ) : (
        groups.map((group) => (
          <section key={group.key} className="mt-6">
            <h2 className="text-muted-foreground mb-1.5 font-serif text-[15px] font-medium">
              {renderGroupLabel(group)}
            </h2>
            <div>
              {group.rows.map((meeting) => {
                const upcoming = isUpcoming(meeting);
                const date = meetingDate(meeting);
                const duration =
                  meeting.started_at && meeting.ended_at
                    ? Math.max(
                        1,
                        Math.round(
                          (Date.parse(meeting.ended_at) -
                            Date.parse(meeting.started_at)) /
                            60000,
                        ),
                      )
                    : null;

                const timeStr = isValid(date) ? format(date, "h:mm a") : "";
                const metaParts: string[] = [];
                if (timeStr) metaParts.push(timeStr);
                if (duration) metaParts.push(`${duration} min`);
                if (meeting.attendees) {
                  const attendeeList = meeting.attendees.split(", ");
                  const preview = attendeeList.slice(0, 3).join(", ");
                  metaParts.push(preview);
                }
                const meetingMeta = metaParts.join(" · ");

                const attendeeCount = meeting.attendee_count;
                const attendeeFacts =
                  attendeeCount > 0
                    ? attendeeCount === 1
                      ? t`${attendeeCount} attendee`
                      : t`${attendeeCount} attendees`
                    : "";

                const statusText = upcoming ? (
                  <Trans>Upcoming</Trans>
                ) : meeting.transcript_count > 0 ? (
                  <Trans>Recorded</Trans>
                ) : (
                  <Trans>Notes only</Trans>
                );

                return (
                  <div
                    key={meeting.id}
                    role="button"
                    tabIndex={0}
                    onClick={() =>
                      openCurrent({ type: "sessions", id: meeting.id })
                    }
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        openCurrent({ type: "sessions", id: meeting.id });
                      }
                    }}
                    className="group border-t border-border/40 py-2 px-1 min-h-[54px] grid grid-cols-[minmax(230px,1.4fr)_minmax(260px,1.3fr)_110px_32px] max-lg:grid-cols-[minmax(180px,1fr)_minmax(180px,1fr)_95px] max-md:grid-cols-[minmax(130px,1fr)_auto] items-center gap-3.5 text-left cursor-pointer hover:bg-accent/30 rounded-xs transition-colors"
                  >
                    <div className="min-w-0">
                      <div className="flex items-center gap-1.5 truncate">
                        <span className="text-[13px] font-medium text-foreground truncate">
                          {meeting.title || <Trans>Untitled</Trans>}
                        </span>
                        {meeting.series_id ? (
                          <RepeatIcon
                            size={13}
                            className="text-muted-foreground shrink-0 inline-block"
                            aria-label={t`Recurring`}
                          />
                        ) : null}
                      </div>
                      <div className="text-muted-foreground text-[11px] mt-0.5 truncate">
                        {meetingMeta}
                      </div>
                    </div>

                    <div className="text-muted-foreground text-[11px] truncate max-md:hidden">
                      {attendeeFacts}
                    </div>

                    <div className="text-muted-foreground text-[11px] whitespace-nowrap text-right justify-self-end">
                      <span>{statusText}</span>
                      {meeting.action_count > 0 ? (
                        <span className="text-muted-foreground text-[10px] ml-2">
                          <Trans>{meeting.action_count} open</Trans>
                        </span>
                      ) : null}
                    </div>

                    <div className="justify-self-end max-lg:hidden">
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          openCurrent({ type: "sessions", id: meeting.id });
                        }}
                        className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 transition-opacity h-[27px] px-2.5 rounded-full border border-border text-foreground hover:bg-accent/50 text-[11px] font-medium shrink-0 flex items-center justify-center cursor-pointer"
                      >
                        <Trans>Open</Trans>
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        ))
      )}
    </div>
  );
}
