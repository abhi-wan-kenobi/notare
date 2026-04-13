import { commands as activityCaptureCommands } from "@hypr/plugin-activity-capture";

export type DailyActivityAppStat = {
  appName: string;
  count: number;
};

export type DailyActivityStats = {
  observationCount: number;
  screenshotCount: number;
  analysisCount: number;
  uniqueAppCount: number;
  firstObservationAtMs: number | null;
  lastObservationAtMs: number | null;
  topApps: DailyActivityAppStat[];
};

export type DailyObservationAnalysis = {
  capturedAtMs: number;
  observationId: string;
  screenshotId: string;
  screenshotKind: string;
  appName: string;
  windowTitle: string | null;
  summary: string;
};

export type DailySummaryTopic = {
  title: string;
  summary: string;
};

export type DailySummaryTimelineItem = {
  time: string;
  summary: string;
};

export type StoredDailySummary = {
  id: string;
  date: string;
  content: string;
  timeline: DailySummaryTimelineItem[];
  topics: DailySummaryTopic[];
  status: string;
  sourceCursorMs: number;
  sourceFingerprint: string;
  generatedAt: string;
  generationError: string;
  updatedAt: string;
};

export type DailySummarySnapshot = {
  stats: DailyActivityStats;
  analyses: DailyObservationAnalysis[];
  summary: StoredDailySummary | null;
  sourceCursorMs: number;
  sourceFingerprint: string;
};

export async function getDailySummarySnapshot(params: {
  date: string;
  startMs: number;
  endMs: number;
}): Promise<DailySummarySnapshot> {
  const result = await activityCaptureCommands.getDailySummarySnapshot(params);

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}

export async function saveDailySummary(params: {
  date: string;
  content: string;
  timeline: DailySummaryTimelineItem[];
  topics: DailySummaryTopic[];
  sourceCursorMs: number;
  sourceFingerprint: string;
  generatedAt: string;
}): Promise<StoredDailySummary> {
  const result = await activityCaptureCommands.saveDailySummary(params);

  if (result.status === "error") {
    throw new Error(String(result.error));
  }

  return result.data;
}
