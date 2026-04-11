import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  commands as activityCaptureCommands,
  events as activityCaptureEvents,
  type ActivityCaptureStatus,
} from "@hypr/plugin-activity-capture";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  getDailySummarySnapshot,
  saveDailySummary,
  type DailySummarySnapshot,
  type DailySummaryTimelineItem,
  type DailySummaryTopic,
} from "./api";
import {
  FeedTabBar,
  StreamEntry,
  StreamFooter,
  TimelineContent,
} from "./components";
import {
  generateDailySummaryOutput,
  getDailySummarySystemPrompt,
  getDailySummaryUserPrompt,
} from "./generate";
import {
  capturedAtDay,
  dateToMsRange,
  entryKey,
  mergeEntries,
  toEntry,
  updateStatus,
  upsertAnalysis,
  upsertEntry,
} from "./helpers";
import {
  type ActivityCaptureEntry,
  type DailySummaryTab,
  type FeedTab,
  type GenerateHandle,
} from "./types";

import { useLanguageModel } from "~/ai/hooks";
import { useTimezone } from "~/calendar/hooks";
import { DateHeader } from "~/main2/home/date-header";
import { StandardTabWrapper } from "~/shared/main";
import { useTabs } from "~/store/zustand/tabs";

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
  const queryClient = useQueryClient();
  const [liveEntries, setLiveEntries] = useState<ActivityCaptureEntry[]>([]);
  const [status, setStatus] = useState<ActivityCaptureStatus | null>(null);
  const [startMs, endMs] = useMemo(() => dateToMsRange(date, tz), [date, tz]);
  const snapshotQueryKey = useMemo(
    () => ["daily-summary-snapshot", date, startMs, endMs] as const,
    [date, startMs, endMs],
  );
  const summaryQuery = useQuery({
    queryKey: snapshotQueryKey,
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

  const entries = useMemo(
    () => mergeEntries(summaryQuery.data?.analyses, liveEntries),
    [liveEntries, summaryQuery.data?.analyses],
  );

  useEffect(() => {
    setLiveEntries([]);
  }, [date]);

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

        setLiveEntries((current) => upsertEntry(current, entry));

        if (payload.type !== "activityCaptureScreenshotAnalysis") {
          return;
        }

        queryClient.setQueryData<DailySummarySnapshot>(
          snapshotQueryKey,
          (current) => {
            if (!current) {
              return current;
            }

            return {
              ...current,
              analyses: upsertAnalysis(current.analyses, {
                capturedAtMs: payload.analysis.capturedAtMs,
                fingerprint: payload.analysis.fingerprint,
                appName: payload.analysis.appName,
                windowTitle: payload.analysis.windowTitle,
                reason: payload.analysis.reason,
                summary: payload.analysis.summary,
              }),
            };
          },
        );
        void queryClient.invalidateQueries({ queryKey: snapshotQueryKey });
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
  }, [date, queryClient, snapshotQueryKey, tz]);

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
      ) : summaryQuery.isPending ? (
        <div className="flex items-center gap-2 py-6 text-sm text-neutral-500">
          <Spinner size={14} className="text-neutral-400" />
          Loading captured activity...
        </div>
      ) : entries.length > 0 ? (
        <div className="-my-1 pt-2">
          {entries.map((entry) => (
            <StreamEntry key={entryKey(entry)} entry={entry} tz={tz} />
          ))}
        </div>
      ) : (
        <div className="py-6 text-sm text-neutral-400">
          No captured activity for this date yet.
        </div>
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

export function TabContentDailySummary({ tab }: { tab: DailySummaryTab }) {
  const updateDailySummaryTabState = useTabs(
    (state) => state.updateDailySummaryTabState,
  );
  const activeTab = tab.state?.activeTab ?? "timeline";
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

  const handleTabChange = useCallback(
    (nextTab: FeedTab) => {
      updateDailySummaryTabState(tab, { activeTab: nextTab });
    },
    [tab, updateDailySummaryTabState],
  );

  return (
    <StandardTabWrapper>
      <div ref={scrollRef} className="h-full overflow-x-hidden overflow-y-auto">
        <div className="sticky top-0 z-10 flex items-center justify-between bg-white px-6 pt-6 pb-3">
          <DateHeader date={tab.id} inline />
          <FeedTabBar
            activeTab={activeTab}
            onTabChange={handleTabChange}
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
