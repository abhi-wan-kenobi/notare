import { Trans, useLingui } from "@lingui/react/macro";
import { format, isValid, parseISO } from "date-fns";
import {
  CheckIcon,
  MinusIcon,
  PlusIcon,
  RepeatIcon,
  UsersIcon,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import {
  parseParticipants,
  prepareSeriesOccurrence,
  type SeriesOccurrence,
  useSeries,
  useSeriesCarryOver,
  useSeriesDelta,
} from "~/services/series";
import { setActionItemStatus } from "~/session/components/action-items/queries";
import { Dock } from "~/shared/dock";
import { StandardContentWrapper } from "~/shared/main";
import { DEFAULT_USER_ID } from "~/shared/utils";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export function TabContentSeries({
  tab,
}: {
  tab: Extract<Tab, { type: "series" }>;
}) {
  const seriesId = tab.state.seriesId;
  return (
    <StandardContentWrapper noBorder>
      <SeriesView seriesId={seriesId} />
    </StandardContentWrapper>
  );
}

function SeriesView({ seriesId }: { seriesId: string }) {
  const { t } = useLingui();
  const { data: occurrences = [] } = useSeries(seriesId);
  const current = occurrences[0];
  const previous = occurrences[1];
  const { data: carryOver = [] } = useSeriesCarryOver(
    seriesId,
    current?.id ?? "",
  );
  const delta = useSeriesDelta(current, previous);
  const openCurrent = useTabs((state) => state.openCurrent);

  const [completedActionIds, setCompletedActionIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isPreparing, setIsPreparing] = useState(false);

  const openSession = useCallback(
    (sessionId: string) => openCurrent({ type: "sessions", id: sessionId }),
    [openCurrent],
  );

  const handlePrepareNext = useCallback(async () => {
    if (isPreparing) return;
    setIsPreparing(true);
    try {
      const nextSessionId = await prepareSeriesOccurrence(seriesId);
      openSession(nextSessionId);
    } catch (error) {
      console.error("[series] failed to prepare next occurrence", error);
    } finally {
      setIsPreparing(false);
    }
  }, [isPreparing, openSession, seriesId]);

  const handleToggleAction = useCallback(
    async (actionId: string) => {
      const isDone = completedActionIds.has(actionId);
      const nextDone = !isDone;
      setCompletedActionIds((prev) => {
        const next = new Set(prev);
        if (nextDone) {
          next.add(actionId);
        } else {
          next.delete(actionId);
        }
        return next;
      });

      try {
        await setActionItemStatus(
          actionId,
          nextDone ? "done" : "todo",
          DEFAULT_USER_ID,
        );
      } catch (error) {
        console.error("[series] failed to toggle action item", error);
      }
    },
    [completedActionIds],
  );

  const title = current?.title || t`Recurring series`;

  const cadence = useMemo(() => computeCadence(occurrences), [occurrences]);

  const oldest = occurrences[occurrences.length - 1];
  const startedText = oldest
    ? formatDateMonthDayLong(oldest.started_at || oldest.created_at)
    : "";

  const attendees = useMemo(() => {
    return parseParticipants(current?.event_json)
      .map((p) => p.name)
      .filter(Boolean);
  }, [current?.event_json]);

  const hasDeltaChanges =
    delta.decisions.length > 0 ||
    delta.actionsClosed.length > 0 ||
    delta.attendeesJoined.length > 0 ||
    delta.attendeesLeft.length > 0 ||
    delta.topicsAdded.length > 0 ||
    delta.topicsRemoved.length > 0;

  return (
    <div className="relative h-full w-full">
      <div className="mx-auto flex h-full w-full max-w-[700px] flex-col overflow-y-auto px-8 py-9">
        {/* Header */}
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <div className="text-muted-foreground text-[11px] font-semibold tracking-wide uppercase">
              <Trans>Recurring meeting series</Trans>
            </div>
            <h1 className="page-title mt-2">{title}</h1>
            <p className="text-muted-foreground mt-1.5 flex items-center gap-1.5 text-xs">
              <RepeatIcon
                size={14}
                className="text-muted-foreground shrink-0"
              />
              <span>
                {cadence ? `${cadence} · ` : ""}
                <Trans>{occurrences.length} meetings</Trans>
                {startedText ? ` · Started ${startedText}` : ""}
              </span>
            </p>
            {attendees.length > 0 && (
              <div className="text-muted-foreground mt-2 flex items-center gap-1.5 text-xs">
                <UsersIcon
                  size={14}
                  className="text-muted-foreground shrink-0"
                />
                <span className="truncate">{attendees.join(", ")}</span>
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={handlePrepareNext}
            disabled={isPreparing}
            className="border-border hover:bg-accent/50 text-foreground shrink-0 rounded-full border px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
          >
            <Trans>Prepare next</Trans>
          </button>
        </div>

        {/* Carried-over actions */}
        <section className="mt-8">
          <div className="border-border flex items-center justify-between border-b pb-2">
            <h2 className="text-muted-foreground text-[11px] font-medium tracking-[0.12em] uppercase">
              <Trans>Carried-over actions</Trans>
            </h2>
            <span className="text-muted-foreground text-xs">
              <Trans>{carryOver.length} still open</Trans>
            </span>
          </div>
          {carryOver.length > 0 ? (
            <div className="divide-border divide-y">
              {carryOver.map((item) => {
                const isDone = completedActionIds.has(item.id);
                return (
                  <div
                    key={item.id}
                    className="flex items-start gap-3 py-3 text-sm"
                  >
                    <input
                      type="checkbox"
                      checked={isDone}
                      onChange={() => handleToggleAction(item.id)}
                      className="border-border text-primary mt-0.5 h-4 w-4 cursor-pointer rounded focus:ring-0"
                    />
                    <div className="min-w-0 flex-1">
                      <div
                        className={cn(
                          "text-foreground font-medium",
                          isDone && "text-muted-foreground line-through",
                        )}
                      >
                        {item.text}
                      </div>
                      <div className="text-muted-foreground mt-0.5 text-xs">
                        {item.assignee_name ? `${item.assignee_name} · ` : ""}
                        <Trans>
                          from {formatDateShort(item.source_started_at)}
                        </Trans>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-muted-foreground py-3 text-sm">
              <Trans>No open items carried over.</Trans>
            </p>
          )}
        </section>

        {/* What changed since last time */}
        <section className="mt-8">
          <div className="border-border flex items-center justify-between border-b pb-2">
            <h2 className="text-muted-foreground text-[11px] font-medium tracking-[0.12em] uppercase">
              <Trans>What changed since last time</Trans>
            </h2>
            {previous && current && (
              <span className="text-muted-foreground text-xs">
                {formatDateShort(previous.started_at || previous.created_at)} →{" "}
                {formatDateShort(current.started_at || current.created_at)}
              </span>
            )}
          </div>
          {hasDeltaChanges ? (
            <div className="divide-border divide-y text-sm">
              {delta.decisions.map((decision, i) => (
                <div key={`dec-${i}`} className="flex items-start gap-3 py-2.5">
                  <PlusIcon
                    size={14}
                    className="mt-0.5 shrink-0 text-green-600"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>Decision made</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      {decision.text}
                    </div>
                  </div>
                </div>
              ))}
              {delta.actionsClosed.map((item) => (
                <div
                  key={`action-${item.id}`}
                  className="flex items-start gap-3 py-2.5"
                >
                  <CheckIcon
                    size={14}
                    className="mt-0.5 shrink-0 text-blue-600"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>Action closed</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      {item.text}
                    </div>
                  </div>
                </div>
              ))}
              {delta.attendeesJoined.map((attendee, i) => (
                <div
                  key={`joined-${i}`}
                  className="flex items-start gap-3 py-2.5"
                >
                  <PlusIcon
                    size={14}
                    className="mt-0.5 shrink-0 text-green-600"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>Attendee joined</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      <Trans>{attendee.name} joined this series.</Trans>
                    </div>
                  </div>
                </div>
              ))}
              {delta.attendeesLeft.map((attendee, i) => (
                <div
                  key={`left-${i}`}
                  className="flex items-start gap-3 py-2.5"
                >
                  <MinusIcon
                    size={14}
                    className="text-muted-foreground mt-0.5 shrink-0"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>Attendee left</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      <Trans>{attendee.name} left this series.</Trans>
                    </div>
                  </div>
                </div>
              ))}
              {delta.topicsAdded.map((topic) => (
                <div
                  key={`added-${topic}`}
                  className="flex items-start gap-3 py-2.5"
                >
                  <PlusIcon
                    size={14}
                    className="mt-0.5 shrink-0 text-green-600"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>New topic</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      {topic}
                    </div>
                  </div>
                </div>
              ))}
              {delta.topicsRemoved.map((topic) => (
                <div
                  key={`removed-${topic}`}
                  className="flex items-start gap-3 py-2.5"
                >
                  <MinusIcon
                    size={14}
                    className="text-muted-foreground mt-0.5 shrink-0"
                  />
                  <div className="min-w-0 flex-1">
                    <strong className="text-foreground">
                      <Trans>Topic no longer active</Trans>
                    </strong>
                    <div className="text-muted-foreground mt-0.5 text-xs">
                      {topic}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-muted-foreground py-3 text-sm">
              <Trans>No changes to show.</Trans>
            </p>
          )}
        </section>

        {/* Timeline */}
        <section className="mt-8">
          <div className="border-border flex items-center justify-between border-b pb-2">
            <h2 className="text-muted-foreground text-[11px] font-medium tracking-[0.12em] uppercase">
              <Trans>Timeline</Trans>
            </h2>
            <span className="text-muted-foreground text-xs">
              <Trans>{occurrences.length} meetings · newest first</Trans>
            </span>
          </div>
          <div className="divide-border divide-y">
            {occurrences.map((occurrence, index) => {
              const isCurrent = index === 0;
              return (
                <button
                  key={occurrence.id}
                  type="button"
                  onClick={() => openSession(occurrence.id)}
                  className={cn(
                    "flex w-full items-start justify-between gap-4 py-3 text-left transition-colors",
                    isCurrent
                      ? "bg-accent/40 -mx-3 rounded-lg px-3"
                      : "hover:bg-accent/30 -mx-3 rounded-lg px-3",
                  )}
                >
                  <span className="text-muted-foreground w-14 shrink-0 pt-0.5 font-mono text-xs font-semibold uppercase">
                    {formatDateMonthDay(
                      occurrence.started_at || occurrence.created_at,
                    )}
                  </span>
                  <div className="min-w-0 flex-1">
                    <span className="text-foreground block truncate text-sm font-medium">
                      {occurrence.title || <Trans>Untitled</Trans>}
                    </span>
                    <span className="text-muted-foreground mt-0.5 flex flex-wrap items-center gap-1.5 text-xs">
                      {occurrence.duration_minutes > 0 && (
                        <span>{occurrence.duration_minutes} min</span>
                      )}
                      {occurrence.duration_minutes > 0 && <span>·</span>}
                      <span>
                        {occurrence.open_item_count === 0 ? (
                          <Trans>No open actions</Trans>
                        ) : (
                          <Trans>
                            {occurrence.open_item_count} open actions
                          </Trans>
                        )}
                      </span>
                      {isCurrent && (
                        <>
                          <span>·</span>
                          <span className="text-foreground font-medium">
                            <Trans>Current meeting</Trans>
                          </span>
                        </>
                      )}
                      {occurrence.tldr && (
                        <>
                          <span>·</span>
                          <span className="max-w-[280px] truncate">
                            {occurrence.tldr}
                          </span>
                        </>
                      )}
                    </span>
                  </div>
                  {isCurrent && (
                    <span className="bg-muted text-muted-foreground shrink-0 rounded px-2 py-0.5 text-[10px] font-semibold tracking-wide uppercase">
                      <Trans>Current</Trans>
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </section>

        <div className="h-16 shrink-0" />
      </div>
      <Dock />
    </div>
  );
}

function computeCadence(occurrences: SeriesOccurrence[]): string {
  if (occurrences.length < 2) return "";
  const d0 = new Date(
    occurrences[0].started_at || occurrences[0].created_at,
  ).getTime();
  const d1 = new Date(
    occurrences[1].started_at || occurrences[1].created_at,
  ).getTime();
  if (Number.isNaN(d0) || Number.isNaN(d1)) return "";
  const diffDays = Math.round(Math.abs(d0 - d1) / 86400000);
  if (diffDays >= 6 && diffDays <= 8) {
    const dayOfWeek = format(new Date(d0), "EEEE");
    return `Every ${dayOfWeek}`;
  }
  if (diffDays >= 13 && diffDays <= 15) {
    return "Bi-weekly";
  }
  if (diffDays >= 27 && diffDays <= 32) {
    return "Monthly";
  }
  if (diffDays === 1) {
    return "Daily";
  }
  if (diffDays > 0) {
    return `Every ${diffDays} days`;
  }
  return "";
}

function formatDateShort(value?: string | null): string {
  if (!value) return "";
  const date = parseISO(value);
  return isValid(date) ? format(date, "MMM d") : "";
}

function formatDateMonthDay(value?: string | null): string {
  if (!value) return "";
  const date = parseISO(value);
  return isValid(date) ? format(date, "MMM d").toUpperCase() : "";
}

function formatDateMonthDayLong(value?: string | null): string {
  if (!value) return "";
  const date = parseISO(value);
  return isValid(date) ? format(date, "MMMM d") : "";
}
