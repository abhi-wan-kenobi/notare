import { useMutation, useQuery } from "@tanstack/react-query";
import { generateText, Output } from "ai";
import { format } from "date-fns";
import { RefreshCwIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { z } from "zod";

import {
  commands as activityCaptureCommands,
  events as activityCaptureEvents,
  type ActivityCapturePluginEvent,
  type ActivityCaptureRuntimeError,
  type ActivityCaptureStatus,
} from "@hypr/plugin-activity-capture";
import { commands as templateCommands } from "@hypr/plugin-template";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import {
  getDailySummarySnapshot,
  saveDailySummary,
  type DailyActivityAnalysis,
  type DailySummarySnapshot,
  type DailySummaryTimelineItem,
  type DailySummaryTopic,
  type StoredDailySummary,
} from "./api";

import { useLanguageModel } from "~/ai/hooks";
import { toTz, useTimezone } from "~/calendar/hooks";
import { DateHeader } from "~/main2/home/date-header";
import { StyledStreamdown } from "~/settings/ai/shared";
import { StandardTabWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

type DailySummaryTab = Extract<Tab, { type: "daily_summary" }>;

const dailySummarySchema = z.object({
  summaryMd: z.string().min(1),
  topics: z
    .array(
      z.object({
        title: z.string().min(1),
        summary: z.string().min(1),
      }),
    )
    .max(8),
  timeline: z
    .array(
      z.object({
        time: z.string().min(1),
        summary: z.string().min(1),
      }),
    )
    .max(12),
});

type ActivityCaptureEntry =
  | {
      kind: "analysis";
      capturedAtMs: number;
      fingerprint: string;
      payload: {
        appName: string;
        windowTitle: string | null;
        summary: string;
        reason: string;
      };
    }
  | {
      kind: "error";
      capturedAtMs: number;
      fingerprint: string;
      payload: {
        appName: string;
        windowTitle: string | null;
        message: string;
      };
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
      payload: {
        appName: payload.analysis.appName,
        windowTitle: payload.analysis.windowTitle,
        summary: payload.analysis.summary,
        reason: payload.analysis.reason,
      },
    };
  }

  if (payload.type === "activityCaptureScreenshotAnalysisError") {
    return {
      kind: "error",
      capturedAtMs: payload.error.capturedAtMs,
      fingerprint: payload.error.fingerprint,
      payload: {
        appName: payload.error.appName,
        windowTitle: payload.error.windowTitle,
        message: payload.error.message,
      },
    };
  }

  return null;
}

function snapshotAnalysisToEntry(
  analysis: DailyActivityAnalysis,
): ActivityCaptureEntry {
  return {
    kind: "analysis",
    capturedAtMs: analysis.capturedAtMs,
    fingerprint: analysis.fingerprint,
    payload: {
      appName: analysis.appName,
      windowTitle: analysis.windowTitle,
      summary: analysis.summary,
      reason: analysis.reason,
    },
  };
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

function formatSignalTime(value: number | null, tz?: string) {
  if (!value) {
    return "—";
  }
  return formatTime(value, tz);
}

function toError(error: unknown) {
  if (error instanceof Error) {
    return error;
  }

  return new Error(String(error));
}

async function getDailySummarySystemPrompt() {
  const result = await templateCommands.render({
    dailySummarySystem: {
      language: null,
    },
  });

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

async function getDailySummaryUserPrompt(params: {
  date: string;
  tz?: string;
  snapshot: DailySummarySnapshot;
}) {
  const { date, tz, snapshot } = params;
  const analyses = snapshot.analyses.slice(-120).map((analysis) => ({
    time: formatTime(analysis.capturedAtMs, tz),
    appName: analysis.appName,
    windowTitle: analysis.windowTitle,
    reason: analysis.reason,
    summary: analysis.summary,
  }));
  const result = await templateCommands.render({
    dailySummaryUser: {
      date,
      timezone: tz ?? null,
      stats: {
        signalCount: snapshot.stats.signalCount,
        screenshotCount: snapshot.stats.screenshotCount,
        analysisCount: snapshot.stats.analysisCount,
        uniqueAppCount: snapshot.stats.uniqueAppCount,
        firstSignal: formatSignalTime(snapshot.stats.firstSignalAtMs, tz),
        lastSignal: formatSignalTime(snapshot.stats.lastSignalAtMs, tz),
      },
      topApps: snapshot.stats.topApps.map((item) => ({
        appName: item.appName,
        count: item.count,
      })),
      analyses,
      totalAnalysisCount: snapshot.analyses.length,
      existingSummary: snapshot.summary?.content?.trim() || null,
    },
  });

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

function createJsonOutputPrompt<T extends z.ZodTypeAny>(
  prompt: string,
  schema: T,
) {
  return `${prompt}

JSON schema:
${JSON.stringify(z.toJSONSchema(schema))}

Return only a JSON object that matches the schema exactly. Start with { and do not wrap the response in markdown fences.`;
}

async function generateDailySummaryOutput(params: {
  model: ReturnType<typeof useLanguageModel>;
  system: string;
  prompt: string;
}) {
  const { model, system, prompt } = params;

  try {
    const result = await generateText({
      model: model!,
      temperature: 0,
      output: Output.object({ schema: dailySummarySchema }),
      system,
      prompt,
    });

    if (!result.output) {
      throw new Error("Model returned no summary.");
    }

    return result.output;
  } catch (error) {
    console.error("Daily summary structured generation failed:", error);

    const fallbackResult = await generateText({
      model: model!,
      temperature: 0,
      system,
      prompt: createJsonOutputPrompt(prompt, dailySummarySchema),
    });
    const jsonMatch = fallbackResult.text.match(/\{[\s\S]*\}/);

    if (!jsonMatch) {
      throw toError(error);
    }

    try {
      return dailySummarySchema.parse(JSON.parse(jsonMatch[0]));
    } catch (parseError) {
      console.error("Daily summary fallback parsing failed:", parseError, {
        text: fallbackResult.text,
      });
      throw toError(error);
    }
  }
}

type FeedTab = "timeline" | "raw";

function FeedTabBar({
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

  return (
    <div className="flex items-center rounded-lg border border-neutral-200 bg-white">
      <button
        type="button"
        onClick={() => onTabChange("timeline")}
        onMouseEnter={() => setHoveringTimeline(true)}
        onMouseLeave={() => setHoveringTimeline(false)}
        className={cn([
          "flex items-center gap-1.5 rounded-l-lg px-3 py-1 text-xs font-medium transition-colors",
          activeTab === "timeline"
            ? "bg-neutral-100 text-neutral-900"
            : "text-neutral-400 hover:text-neutral-600",
        ])}
      >
        Timeline
        {activeTab === "timeline" && hoveringTimeline && onGenerate && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onGenerate();
            }}
            disabled={!canGenerate || isGenerating}
            className="text-neutral-400 hover:text-neutral-900 disabled:opacity-50"
          >
            <RefreshCwIcon
              className={cn(["h-3 w-3", isGenerating && "animate-spin"])}
            />
          </button>
        )}
      </button>
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

function TimelineContent({
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

function ActivityCaptureFeed({
  date,
  activeTab,
  generateRef,
  onGenerateStateChange,
}: {
  date: string;
  activeTab: FeedTab;
  generateRef: React.MutableRefObject<GenerateHandle>;
  onGenerateStateChange: () => void;
}) {
  const tz = useTimezone();
  const model = useLanguageModel("enhance");
  const [entries, setEntries] = useState<ActivityCaptureEntry[]>([]);
  const [status, setStatus] = useState<ActivityCaptureStatus | null>(null);
  const [startMs, endMs] = useMemo(() => dateToMsRange(date, tz), [date, tz]);
  const summaryQuery = useQuery({
    queryKey: ["daily-summary-snapshot", date, startMs, endMs],
    queryFn: () => getDailySummarySnapshot({ date, startMs, endMs }),
  });
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
  const generateMutation = useMutation({
    mutationFn: async () => {
      if (!model) {
        throw new Error("No language model configured.");
      }

      const snapshot =
        (await summaryQuery.refetch()).data ?? summaryQuery.data ?? null;
      if (!snapshot) {
        throw new Error("Failed to load daily summary source data.");
      }
      if (snapshot.analyses.length === 0) {
        throw new Error("No analyzed activity is available for this date.");
      }

      const [system, prompt] = await Promise.all([
        getDailySummarySystemPrompt(),
        getDailySummaryUserPrompt({ date, tz, snapshot }),
      ]);
      const result = await generateDailySummaryOutput({
        model,
        system,
        prompt,
      });

      await saveDailySummary({
        date,
        content: result.summaryMd,
        timeline: result.timeline as DailySummaryTimelineItem[],
        topics: result.topics as DailySummaryTopic[],
        sourceCursorMs: snapshot.sourceCursorMs,
        sourceFingerprint: snapshot.sourceFingerprint,
        generatedAt: new Date().toISOString(),
      });
    },
    onSuccess: async () => {
      await summaryQuery.refetch();
      sonnerToast.success("Daily summary updated.");
    },
    onError: (error) => {
      console.error("Failed to generate daily summary:", error);
      sonnerToast.error(error.message);
    },
  });

  useEffect(() => {
    if (!summaryQuery.data) {
      setEntries([]);
      return;
    }

    setEntries(summaryQuery.data.analyses.map(snapshotAnalysisToEntry));
  }, [date, summaryQuery.data]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    activityCaptureCommands
      .status()
      .then((statusResult) => {
        if (!cancelled && statusResult.status === "ok") {
          setStatus(statusResult.data);
        }
      })
      .catch((error) => {
        console.error("Failed to load activity capture status:", error);
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

  const canGenerate = !!model && (summaryQuery.data?.analyses.length ?? 0) > 0;

  useEffect(() => {
    generateRef.current = {
      mutate: () => generateMutation.mutate(),
      isPending: generateMutation.isPending,
      canGenerate,
    };
    onGenerateStateChange();
  }, [
    generateMutation.isPending,
    canGenerate,
    generateRef,
    onGenerateStateChange,
  ]);

  return (
    <section className="px-6">
      {activeTab === "timeline" ? (
        <TimelineContent
          summary={summaryQuery.data?.summary ?? null}
          isLoading={summaryQuery.isPending}
          isGenerating={generateMutation.isPending}
        />
      ) : (
        entries.length > 0 && (
          <div className="-my-1 pt-2">
            {entries.map((entry) => (
              <StreamEntry key={entryKey(entry)} entry={entry} tz={tz} />
            ))}
          </div>
        )
      )}

      <div className="sticky bottom-0 pt-2 pb-4">
        <StreamFooter
          status={status}
          isPending={captureMutation.isPending || generateMutation.isPending}
          onToggle={() => captureMutation.mutate(!status?.isRunning)}
        />
      </div>
    </section>
  );
}

function renderRuntimeError(error: ActivityCaptureRuntimeError) {
  return `${readableReason(error.kind)}: ${error.message}`;
}

interface GenerateHandle {
  mutate: () => void;
  isPending: boolean;
  canGenerate: boolean;
}

export function TabContentDailySummary({ tab }: { tab: DailySummaryTab }) {
  const [activeTab, setActiveTab] = useState<FeedTab>("timeline");
  const generateRef = useRef<GenerateHandle>({
    mutate: () => {},
    isPending: false,
    canGenerate: false,
  });
  const [, forceRender] = useState(0);
  const scrollRef = useRef<HTMLDivElement>(null);
  const handleGenerateStateChange = useCallback(() => {
    forceRender((n) => n + 1);
  }, []);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [activeTab]);

  return (
    <StandardTabWrapper>
      <div ref={scrollRef} className="h-full overflow-x-hidden overflow-y-auto">
        <div className="sticky top-0 z-10 flex items-center justify-between bg-white px-6 pt-6 pb-3">
          <DateHeader date={tab.id} inline />
          <FeedTabBar
            activeTab={activeTab}
            onTabChange={setActiveTab}
            isGenerating={generateRef.current.isPending}
            canGenerate={generateRef.current.canGenerate}
            onGenerate={() => generateRef.current.mutate()}
          />
        </div>
        <ActivityCaptureFeed
          date={tab.id}
          activeTab={activeTab}
          generateRef={generateRef}
          onGenerateStateChange={handleGenerateStateChange}
        />
      </div>
    </StandardTabWrapper>
  );
}
