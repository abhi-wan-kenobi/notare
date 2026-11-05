import { Trans } from "@lingui/react/macro";
import { ChevronRightIcon, Repeat2Icon, UsersIcon } from "lucide-react";
import { useMemo } from "react";

import type { SessionEvent } from "@hypr/store";
import { cn } from "@hypr/utils";

import { useChapters } from "./chapters";
import { useSessionActionItems } from "./components/action-items/queries";
import {
  type SessionDecisionRecord,
  type SessionParticipantRecord,
  type SessionRecord,
  useSessionDecisions,
} from "./queries";
import { useSessionSeriesMeta } from "./series-meta";

import { useTabs } from "~/store/zustand/tabs";

export type MeetingDetailTab =
  | "summary"
  | "transcript"
  | "action-items"
  | "recording";

export function MeetingDetail({
  session,
  event,
  participants,
  rawNotes,
  enhancedMarkdown,
  activeTab,
  onTabChange,
  onSeek,
  children,
}: {
  session: SessionRecord;
  event: SessionEvent | null;
  participants: SessionParticipantRecord[];
  rawNotes: string;
  enhancedMarkdown: string;
  activeTab: MeetingDetailTab;
  onTabChange: (tab: MeetingDetailTab) => void;
  onSeek: (ms: number) => void;
  children: React.ReactNode;
}) {
  const series = useSessionSeriesMeta(session.id);
  const chapters = useChapters(session.id, enhancedMarkdown);
  const decisions = useSessionDecisions(session.id);
  const { items = [] } = useSessionActionItems(session.id);
  const openActionCount = useMemo(
    () => items.filter((item) => item.status !== "done").length,
    [items],
  );
  const atAGlance = useMemo(
    () => parseAtAGlance(enhancedMarkdown),
    [enhancedMarkdown],
  );
  const openNew = useTabs((state) => state.openNew);
  const date = event?.started_at
    ? new Date(event.started_at)
    : new Date(session.created_at);
  const end = event?.ended_at ? new Date(event.ended_at) : null;
  const attendees = participants
    .map((participant) => participant.name)
    .filter(Boolean);

  const selectChapter = (startMs: number) => {
    onSeek(startMs);
    onTabChange("transcript");
    requestAnimationFrame(() => {
      const target = Array.from(
        document.querySelectorAll<HTMLElement>("[data-word-start-ms]"),
      ).find((node) => Number(node.dataset.wordStartMs) >= startMs);
      target?.scrollIntoView({ block: "center", behavior: "smooth" });
    });
  };

  return (
    <>
      <header className="mx-auto w-full max-w-[760px] px-6 pt-6 pb-2">
        <div className="flex items-start gap-4">
          <div className="bg-muted/70 flex h-[52px] w-12 shrink-0 flex-col items-center justify-center rounded-[9px] text-center">
            <span className="text-muted-foreground text-[10px] leading-tight font-semibold uppercase">
              {date.toLocaleDateString(undefined, { weekday: "short" })}
            </span>
            <span className="text-foreground text-[19px] leading-tight font-semibold tabular-nums">
              {date.getDate()}
            </span>
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="text-foreground font-serif text-[29px] leading-[1.15] font-medium tracking-[-0.025em]">
              {session.title || event?.title || <Trans>Untitled meeting</Trans>}
            </h1>
            <div className="text-muted-foreground mt-1.5 flex flex-wrap items-center gap-1.5 text-xs">
              <span>
                {date.toLocaleDateString(undefined, {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </span>
              {event?.started_at ? (
                <>
                  <span>·</span>
                  <span>
                    {eventTime(event.started_at)}
                    {end ? `–${eventTime(event.ended_at)}` : ""}
                  </span>
                </>
              ) : null}
              {attendees.length ? (
                <>
                  <span>·</span>
                  <span className="inline-flex items-center gap-1">
                    <UsersIcon size={13} className="shrink-0" />
                    {attendees.length}{" "}
                    <Trans>
                      {attendees.length === 1 ? "attendee" : "attendees"}
                    </Trans>
                  </span>
                </>
              ) : null}
            </div>
            {series ? (
              <button
                type="button"
                className="text-muted-foreground hover:text-foreground mt-2 flex items-center gap-1.5 text-xs transition-colors"
                onClick={() =>
                  openNew({ type: "series", seriesId: series.seriesId })
                }
              >
                <Repeat2Icon size={14} className="shrink-0" />
                <span>
                  {series.cadence ? (
                    <>
                      {series.cadence === "Weekly" ? (
                        <Trans>Weekly</Trans>
                      ) : series.cadence === "Biweekly" ? (
                        <Trans>Every two weeks</Trans>
                      ) : (
                        <Trans>Monthly</Trans>
                      )}{" "}
                      <Trans>on</Trans>{" "}
                      {date.toLocaleDateString(undefined, { weekday: "long" })}
                    </>
                  ) : (
                    <Trans>Recurring</Trans>
                  )}{" "}
                  · {series.meetingCount} <Trans>meetings</Trans> ·{" "}
                  <Trans>since</Trans>{" "}
                  {new Date(series.firstStartedAt).toLocaleDateString(
                    undefined,
                    { month: "short", day: "numeric" },
                  )}
                </span>
                <ChevronRightIcon size={14} className="shrink-0" />
              </button>
            ) : null}
          </div>
        </div>

        <nav
          aria-label="Meeting sections"
          className="border-border/70 mt-6 flex gap-1 border-b pb-2"
        >
          {(
            [
              ["summary", <Trans>Summary</Trans>],
              ["transcript", <Trans>Transcript</Trans>],
              [
                "action-items",
                <>
                  <Trans>Action items</Trans>
                  {openActionCount > 0 ? (
                    <span className="bg-muted text-muted-foreground ml-1.5 rounded-full px-1.5 py-0.5 text-[10px] font-semibold tabular-nums">
                      {openActionCount}
                    </span>
                  ) : null}
                </>,
              ],
              ["recording", <Trans>Recording</Trans>],
            ] as const
          ).map(([tab, label]) => (
            <button
              key={tab}
              type="button"
              aria-current={activeTab === tab ? "page" : undefined}
              onClick={() => onTabChange(tab)}
              className={cn(
                "h-8 rounded-[7px] px-3 text-xs font-medium transition-colors",
                activeTab === tab
                  ? "bg-accent text-foreground font-semibold"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </nav>
      </header>

      {activeTab === "summary" ? (
        <main className="mx-auto min-h-0 w-full max-w-[760px] flex-1 overflow-y-auto px-6 pb-24">
          {rawNotes.trim() ? (
            <section className="text-foreground mb-5 text-sm leading-6 font-medium whitespace-pre-wrap">
              {rawNotes.trim()}
            </section>
          ) : null}

          {atAGlance ? (
            <section className="border-border/60 mb-5 border-b pb-5">
              <h2 className="text-muted-foreground mb-3 text-[10px] font-semibold tracking-[0.1em] uppercase">
                <Trans>At a glance</Trans>
              </h2>
              {atAGlance.lead ? (
                <div className="text-secondary-foreground mb-3 text-sm leading-relaxed whitespace-pre-wrap">
                  {atAGlance.lead}
                </div>
              ) : null}
              {atAGlance.bullets.length ? (
                <div className="flex flex-col gap-1.5">
                  {atAGlance.bullets.map((bullet, i) => (
                    <div
                      key={i}
                      className="text-secondary-foreground relative pl-4 text-sm leading-relaxed before:absolute before:top-2 before:left-1 before:size-1 before:rounded-full before:bg-neutral-400"
                    >
                      {bullet}
                    </div>
                  ))}
                </div>
              ) : null}
            </section>
          ) : null}

          {chapters.length ? (
            <section className="border-border/60 mb-5 border-b pb-4">
              <h2 className="text-muted-foreground mb-2 text-[10px] font-semibold tracking-[0.1em] uppercase">
                <Trans>Chapters</Trans>
              </h2>
              <div className="flex flex-col">
                {chapters.map((chapter) => (
                  <button
                    key={`${chapter.title}-${chapter.startMs}`}
                    type="button"
                    onClick={() => selectChapter(chapter.startMs)}
                    className="hover:bg-accent/40 border-border/40 flex w-full items-center gap-3 border-t py-2 text-left text-sm first:border-t-0"
                  >
                    <span className="text-muted-foreground w-11 shrink-0 font-mono text-xs tabular-nums">
                      {formatTimestamp(chapter.startMs)}
                    </span>
                    <span className="text-foreground min-w-0 flex-1 truncate">
                      {chapter.title}
                    </span>
                    <span className="text-muted-foreground font-mono text-xs tabular-nums">
                      {formatDuration(chapter.durationMs)}
                    </span>
                  </button>
                ))}
              </div>
            </section>
          ) : null}

          {decisions.length ? (
            <section className="border-border/60 mb-5 border-b pb-4">
              <h2 className="text-muted-foreground mb-2 text-[10px] font-semibold tracking-[0.1em] uppercase">
                <Trans>Decisions</Trans>
              </h2>
              <div className="flex flex-col">
                {decisions.map((decision) => (
                  <DecisionRow
                    key={decision.id}
                    decision={decision}
                    onSeek={onSeek}
                  />
                ))}
              </div>
            </section>
          ) : null}

          {children}
        </main>
      ) : (
        <div className="min-h-0 flex-1">{children}</div>
      )}
    </>
  );
}

function DecisionRow({
  decision,
  onSeek,
}: {
  decision: SessionDecisionRecord;
  onSeek: (ms: number) => void;
}) {
  return (
    <div className="border-border/40 flex items-center gap-3 border-t py-2.5 text-sm first:border-t-0">
      <span className="border-border/80 text-muted-foreground rounded-md border px-1.5 py-0.5 text-[10px] font-medium tracking-wide uppercase">
        <Trans>Decision</Trans>
      </span>
      {decision.atMs !== null ? (
        <button
          type="button"
          onClick={() => onSeek(decision.atMs!)}
          className="text-muted-foreground font-mono text-xs tabular-nums hover:underline"
        >
          {formatTimestamp(decision.atMs)}
        </button>
      ) : null}
      <span className="text-foreground min-w-0 flex-1">{decision.text}</span>
      {decision.owner ? (
        <span className="text-muted-foreground shrink-0 text-xs">
          {decision.owner}
        </span>
      ) : null}
    </div>
  );
}

function parseAtAGlance(markdown: string): {
  lead: string;
  bullets: string[];
} | null {
  const withoutH1 = markdown.replace(/^#\s+[^\n]*\n+/, "");
  const h2Match = /\n##\s+/.exec(withoutH1);
  const intro = h2Match
    ? withoutH1.slice(0, h2Match.index).trim()
    : withoutH1.trim();
  if (!intro) return null;

  const lines = intro
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  const bullets: string[] = [];
  const leadLines: string[] = [];

  for (const line of lines) {
    if (line.startsWith("- ") || line.startsWith("* ")) {
      bullets.push(line.slice(2).trim());
    } else if (!line.startsWith("```")) {
      leadLines.push(line);
    }
  }

  if (!leadLines.length && !bullets.length) return null;

  return {
    lead: leadLines.join("\n\n"),
    bullets,
  };
}

function eventTime(value: string) {
  return new Date(value).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

function formatTimestamp(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

function formatDuration(ms: number) {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}
