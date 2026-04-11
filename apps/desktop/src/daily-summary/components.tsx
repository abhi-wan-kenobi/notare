import { RefreshCwIcon } from "lucide-react";
import { useState } from "react";

import { type ActivityCaptureStatus } from "@hypr/plugin-activity-capture";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { type StoredDailySummary } from "./api";
import {
  appContext,
  formatTime,
  readableReason,
  renderRuntimeError,
} from "./helpers";
import { type ActivityCaptureEntry, type FeedTab } from "./types";

import { StyledStreamdown } from "~/settings/ai/shared";

export function FeedTabBar({
  activeTab,
  onTabChange,
  isGenerating,
  canGenerate,
  onGenerate,
}: {
  activeTab: FeedTab;
  onTabChange: (tab: FeedTab) => void;
  isGenerating?: boolean;
  canGenerate?: boolean;
  onGenerate?: () => void;
}) {
  const [hoveringTimeline, setHoveringTimeline] = useState(false);
  const isTimelineActive = activeTab === "timeline";

  return (
    <div className="flex items-center rounded-lg border border-neutral-200 bg-white">
      <div
        onMouseEnter={() => setHoveringTimeline(true)}
        onMouseLeave={() => setHoveringTimeline(false)}
        className={cn([
          "flex items-center rounded-l-lg",
          isTimelineActive && "bg-neutral-100",
        ])}
      >
        <button
          type="button"
          onClick={() => onTabChange("timeline")}
          className={cn([
            "px-3 py-1 text-xs font-medium transition-colors",
            isTimelineActive
              ? "text-neutral-900"
              : "text-neutral-400 hover:text-neutral-600",
          ])}
        >
          Timeline
        </button>
        <div className="flex w-6 items-center justify-center pr-1">
          <button
            type="button"
            onClick={onGenerate}
            disabled={
              !isTimelineActive ||
              !hoveringTimeline ||
              !onGenerate ||
              !canGenerate ||
              isGenerating
            }
            className={cn([
              "text-neutral-400 transition-all hover:text-neutral-900 disabled:cursor-default disabled:opacity-50",
              isTimelineActive && hoveringTimeline && onGenerate
                ? "pointer-events-auto opacity-100"
                : "pointer-events-none opacity-0",
            ])}
            aria-hidden={!isTimelineActive || !hoveringTimeline}
            tabIndex={isTimelineActive && hoveringTimeline ? 0 : -1}
          >
            <RefreshCwIcon
              className={cn(["h-3 w-3", isGenerating && "animate-spin"])}
            />
          </button>
        </div>
      </div>
      <button
        type="button"
        onClick={() => onTabChange("raw")}
        className={cn([
          "rounded-r-lg px-3 py-1 text-xs font-medium transition-colors",
          activeTab === "raw"
            ? "bg-neutral-100 text-neutral-900"
            : "text-neutral-400 hover:text-neutral-600",
        ])}
      >
        Raw
      </button>
    </div>
  );
}

export function TimelineContent({
  summary,
  isLoading,
  isGenerating,
}: {
  summary: StoredDailySummary | null;
  isLoading: boolean;
  isGenerating: boolean;
}) {
  if (isLoading) {
    return (
      <div className="flex items-center gap-2 py-6 text-sm text-neutral-500">
        <Spinner size={14} className="text-neutral-400" />
        Loading...
      </div>
    );
  }

  if (isGenerating) {
    return (
      <div className="flex items-center gap-2 py-6 text-sm text-neutral-500">
        <Spinner size={14} className="text-neutral-400" />
        Generating summary...
      </div>
    );
  }

  if (!summary) {
    return (
      <div className="py-6 text-sm text-neutral-400">
        No summary yet. Hover over the Timeline tab and click the refresh icon
        to generate.
      </div>
    );
  }

  return (
    <div className="space-y-4 py-4">
      <StyledStreamdown>{summary.content}</StyledStreamdown>

      {summary.topics.length ? (
        <div className="space-y-2">
          <div className="text-xs font-medium tracking-[0.08em] text-neutral-400 uppercase">
            Topics
          </div>
          <div className="grid gap-2 md:grid-cols-2">
            {summary.topics.map((topic) => (
              <div
                key={topic.title}
                className="rounded-xl border border-neutral-200 bg-neutral-50 px-3 py-2"
              >
                <div className="truncate text-sm font-medium text-neutral-900">
                  {topic.title}
                </div>
                <p className="mt-0.5 line-clamp-2 text-xs text-neutral-600">
                  {topic.summary}
                </p>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {summary.timeline.length ? (
        <div className="space-y-2">
          <div className="text-xs font-medium tracking-[0.08em] text-neutral-400 uppercase">
            Timeline
          </div>
          <div className="space-y-2">
            {summary.timeline.map((item) => (
              <div
                key={`${item.time}-${item.summary}`}
                className="flex gap-3 rounded-xl border border-neutral-200 px-3 py-3"
              >
                <div className="w-16 shrink-0 text-xs text-neutral-400 tabular-nums">
                  {item.time}
                </div>
                <div className="text-sm text-neutral-700">{item.summary}</div>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

export function StreamEntry({
  entry,
  tz,
}: {
  entry: ActivityCaptureEntry;
  tz?: string;
}) {
  if (entry.kind === "analysis") {
    return (
      <div className="group relative flex gap-3 py-3">
        <div className="flex w-16 shrink-0 flex-col items-end pt-0.5">
          <span className="text-xs text-neutral-400 tabular-nums">
            {formatTime(entry.capturedAtMs, tz)}
          </span>
        </div>
        <div className="relative flex-1 border-l border-neutral-200 pl-4">
          <div className="absolute top-1.5 -left-[5px] h-2.5 w-2.5 rounded-full border-2 border-emerald-400 bg-white" />
          <div className="text-sm font-medium text-neutral-900">
            {appContext(entry.payload.appName, entry.payload.windowTitle)}
          </div>
          <p className="mt-1 text-sm leading-relaxed text-neutral-600">
            {entry.payload.summary}
          </p>
          <div className="mt-2">
            <span className="text-[11px] text-neutral-400">
              {readableReason(entry.payload.reason)}
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="group relative flex gap-3 py-3">
      <div className="flex w-16 shrink-0 flex-col items-end pt-0.5">
        <span className="text-xs text-neutral-400 tabular-nums">
          {formatTime(entry.capturedAtMs, tz)}
        </span>
      </div>
      <div className="relative flex-1 border-l border-red-200 pl-4">
        <div className="absolute top-1.5 -left-[5px] h-2.5 w-2.5 rounded-full border-2 border-red-400 bg-white" />
        <div className="text-sm font-medium text-neutral-900">
          {appContext(entry.payload.appName, entry.payload.windowTitle)}
        </div>
        <p className="mt-1 text-sm leading-relaxed text-red-600">
          {entry.payload.message}
        </p>
      </div>
    </div>
  );
}

export function StreamFooter({
  status,
  isPending,
  onToggle,
}: {
  status: ActivityCaptureStatus | null;
  isPending: boolean;
  onToggle: () => void;
}) {
  const hasError = !!status?.lastError;

  return (
    <div
      className={cn([
        "mt-2 flex items-center gap-3 rounded-lg px-3 py-2",
        hasError ? "bg-red-50" : "bg-neutral-100",
      ])}
    >
      <div
        className={cn([
          "h-1.5 w-1.5 rounded-full",
          status?.isRunning ? "bg-emerald-500" : "bg-neutral-300",
        ])}
      />
      <span className="flex-1 text-xs text-neutral-500">
        {isPending
          ? "Updating..."
          : hasError
            ? `Error: ${renderRuntimeError(status!.lastError!)}`
            : status?.isRunning
              ? "Capturing activity"
              : "Capture paused"}
      </span>
      <button
        type="button"
        onClick={onToggle}
        disabled={isPending}
        className={cn([
          "text-xs font-medium transition-colors disabled:opacity-50",
          status?.isRunning
            ? "text-neutral-500 hover:text-neutral-700"
            : "text-neutral-900 hover:text-neutral-700",
        ])}
      >
        {status?.isRunning ? "Stop" : "Resume"}
      </button>
    </div>
  );
}
