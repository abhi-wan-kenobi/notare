import { useMutation } from "@tanstack/react-query";
import { format } from "date-fns";
import { useEffect, useState } from "react";

import {
  commands as activityCaptureCommands,
  events as activityCaptureEvents,
  type ActivityCapturePluginEvent,
  type ActivityCaptureRuntimeError,
  type ActivityCaptureScreenshotAnalysis,
  type ActivityCaptureScreenshotAnalysisError,
  type ActivityCaptureStatus,
} from "@hypr/plugin-activity-capture";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { toTz, useTimezone } from "~/calendar/hooks";
import { DateHeader } from "~/main2/home/date-header";
import { StandardTabWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

type DailySummaryTab = Extract<Tab, { type: "daily_summary" }>;

type ActivityCaptureEntry =
  | {
      kind: "analysis";
      capturedAtMs: number;
      fingerprint: string;
      payload: ActivityCaptureScreenshotAnalysis;
    }
  | {
      kind: "error";
      capturedAtMs: number;
      fingerprint: string;
      payload: ActivityCaptureScreenshotAnalysisError;
    };

function capturedAtDay(capturedAtMs: number, tz?: string) {
  return format(toTz(new Date(capturedAtMs), tz), "yyyy-MM-dd");
}

function formatTime(capturedAtMs: number, tz?: string) {
  return format(toTz(new Date(capturedAtMs), tz), "HH:mm:ss");
}

function entryKey(entry: ActivityCaptureEntry) {
  return `${entry.kind}:${entry.fingerprint}:${entry.capturedAtMs}`;
}

function upsertEntry(
  current: ActivityCaptureEntry[],
  next: ActivityCaptureEntry,
): ActivityCaptureEntry[] {
  const nextKey = entryKey(next);
  return [next, ...current.filter((entry) => entryKey(entry) !== nextKey)].sort(
    (a, b) => a.capturedAtMs - b.capturedAtMs,
  );
}

function dateToMsRange(date: string, tz?: string): [number, number] {
  const dayStart = toTz(new Date(`${date}T00:00:00`), tz);
  const dayEnd = new Date(dayStart.getTime());
  dayEnd.setDate(dayEnd.getDate() + 1);
  return [dayStart.getTime(), dayEnd.getTime()];
}

function toEntry(
  payload: ActivityCapturePluginEvent,
): ActivityCaptureEntry | null {
  if (payload.type === "activityCaptureScreenshotAnalysis") {
    return {
      kind: "analysis",
      capturedAtMs: payload.analysis.capturedAtMs,
      fingerprint: payload.analysis.fingerprint,
      payload: payload.analysis,
    };
  }

  if (payload.type === "activityCaptureScreenshotAnalysisError") {
    return {
      kind: "error",
      capturedAtMs: payload.error.capturedAtMs,
      fingerprint: payload.error.fingerprint,
      payload: payload.error,
    };
  }

  return null;
}

function emptyStatus(): ActivityCaptureStatus {
  return {
    isRunning: false,
    lastStateChangedAtMs: null,
    lastSignal: null,
    lastError: null,
    lastScreenshotAnalysis: null,
    lastScreenshotAnalysisError: null,
    budget: {
      minIntervalSecs: 0,
    },
    analyzeScreenshots: false,
    screenshotsToday: 0,
    screenshotsThisHour: 0,
    storageUsedMb: 0,
  };
}

function updateStatus(
  current: ActivityCaptureStatus | null,
  payload: ActivityCapturePluginEvent,
): ActivityCaptureStatus {
  const next = current ?? emptyStatus();

  if (payload.type === "activityCaptureStateChanged") {
    return {
      ...next,
      isRunning: payload.state.isRunning,
      lastStateChangedAtMs: payload.state.changedAtMs,
    };
  }

  if (payload.type === "activityCaptureSignal") {
    return {
      ...next,
      lastSignal: payload.signal,
    };
  }

  if (payload.type === "activityCaptureError") {
    return {
      ...next,
      isRunning: false,
      lastStateChangedAtMs: payload.error.occurredAtMs,
      lastError: payload.error,
    };
  }

  if (payload.type === "activityCaptureScreenshotAnalysis") {
    return {
      ...next,
      lastScreenshotAnalysis: payload.analysis,
      lastScreenshotAnalysisError: null,
    };
  }

  if (payload.type === "activityCaptureScreenshotAnalysisError") {
    return {
      ...next,
      lastScreenshotAnalysisError: payload.error,
    };
  }

  return next;
}

function readableReason(reason: string) {
  return reason.split("_").join(" ");
}

function appContext(appName: string, windowTitle: string | null) {
  return windowTitle ? `${appName} · ${windowTitle}` : appName;
}

function StreamEntry({
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

function StreamFooter({
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
        hasError ? "bg-red-50" : "bg-neutral-50",
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

function ActivityCaptureFeed({ date }: { date: string }) {
  const tz = useTimezone();
  const [entries, setEntries] = useState<ActivityCaptureEntry[]>([]);
  const [status, setStatus] = useState<ActivityCaptureStatus | null>(null);
  const captureMutation = useMutation({
    mutationFn: async (nextRunning: boolean) => {
      const result = nextRunning
        ? await activityCaptureCommands.start()
        : await activityCaptureCommands.stop();

      if (result.status === "error") {
        throw new Error(String(result.error));
      }
    },
    onSuccess: async () => {
      const result = await activityCaptureCommands.status();
      if (result.status !== "ok") {
        return;
      }

      setStatus(result.data);
    },
    onError: (error) => {
      sonnerToast.error(error.message);
    },
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    setEntries([]);

    const [startMs, endMs] = dateToMsRange(date, tz);

    Promise.all([
      activityCaptureCommands.status(),
      activityCaptureCommands.listAnalysesInRange(startMs, endMs),
    ])
      .then(([statusResult, analysesResult]) => {
        if (cancelled) {
          return;
        }

        if (statusResult.status === "ok") {
          setStatus(statusResult.data);
        }

        if (analysesResult.status === "ok") {
          const historical: ActivityCaptureEntry[] = analysesResult.data.map(
            (analysis) => ({
              kind: "analysis" as const,
              capturedAtMs: analysis.capturedAtMs,
              fingerprint: analysis.fingerprint,
              payload: analysis,
            }),
          );
          setEntries(historical);
        }
      })
      .catch((error) => {
        console.error("Failed to load activity capture data:", error);
      });

    activityCaptureEvents.activityCapturePluginEvent
      .listen(({ payload }) => {
        setStatus((current) => updateStatus(current, payload));

        const entry = toEntry(payload);
        if (!entry) {
          return;
        }

        if (capturedAtDay(entry.capturedAtMs, tz) !== date) {
          return;
        }

        setEntries((current) => upsertEntry(current, entry));
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((error) => {
        console.error("Failed to listen to activity capture events:", error);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [date, tz]);

  return (
    <section className="px-6">
      {entries.length > 0 && (
        <div className="-my-1">
          {entries.map((entry) => (
            <StreamEntry key={entryKey(entry)} entry={entry} tz={tz} />
          ))}
        </div>
      )}

      <div className="sticky bottom-0 pt-2 pb-4">
        <StreamFooter
          status={status}
          isPending={captureMutation.isPending}
          onToggle={() => captureMutation.mutate(!status?.isRunning)}
        />
      </div>
    </section>
  );
}

function renderRuntimeError(error: ActivityCaptureRuntimeError) {
  return `${readableReason(error.kind)}: ${error.message}`;
}

export function TabContentDailySummary({ tab }: { tab: DailySummaryTab }) {
  return (
    <StandardTabWrapper>
      <div className="h-full overflow-auto">
        <div className="flex items-center justify-between px-6 pt-6 pb-3">
          <DateHeader date={tab.id} inline />
        </div>
        <ActivityCaptureFeed date={tab.id} />
      </div>
    </StandardTabWrapper>
  );
}
