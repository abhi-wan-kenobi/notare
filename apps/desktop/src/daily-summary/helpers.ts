import { format } from "date-fns";

import {
  type ActivityCapturePluginEvent,
  type ActivityCaptureRuntimeError,
  type ActivityCaptureStatus,
} from "@hypr/plugin-activity-capture";

import { type DailyObservationAnalysis } from "./api";
import { type ActivityCaptureEntry } from "./types";

import { toTz } from "~/calendar/hooks";

export function capturedAtDay(capturedAtMs: number, tz?: string) {
  return format(toTz(new Date(capturedAtMs), tz), "yyyy-MM-dd");
}

export function formatTime(capturedAtMs: number, tz?: string) {
  return format(toTz(new Date(capturedAtMs), tz), "HH:mm:ss");
}

export function entryKey(entry: ActivityCaptureEntry) {
  return `${entry.kind}:${entry.id}:${entry.capturedAtMs}`;
}

export function upsertEntry(
  current: ActivityCaptureEntry[],
  next: ActivityCaptureEntry,
): ActivityCaptureEntry[] {
  const nextKey = entryKey(next);
  return [next, ...current.filter((entry) => entryKey(entry) !== nextKey)].sort(
    (a, b) => a.capturedAtMs - b.capturedAtMs,
  );
}

export function dateToMsRange(date: string, tz?: string): [number, number] {
  const dayStart = toTz(new Date(`${date}T00:00:00`), tz);
  const dayEnd = new Date(dayStart.getTime());
  dayEnd.setDate(dayEnd.getDate() + 1);
  return [dayStart.getTime(), dayEnd.getTime()];
}

export function toEntry(
  payload: ActivityCapturePluginEvent,
): ActivityCaptureEntry | null {
  if (payload.type === "activityObservationAnalysisReady") {
    return {
      kind: "analysis",
      capturedAtMs: payload.analysis.capturedAtMs,
      id: payload.analysis.screenshotId,
      payload: {
        appName: payload.analysis.appName,
        windowTitle: payload.analysis.windowTitle,
        summary: payload.analysis.summary,
        reason: payload.analysis.screenshotKind,
      },
    };
  }

  if (payload.type === "activityObservationAnalysisError") {
    return {
      kind: "error",
      capturedAtMs: payload.error.capturedAtMs,
      id: payload.error.screenshotId,
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
  analysis: DailyObservationAnalysis,
): ActivityCaptureEntry {
  return {
    kind: "analysis",
    capturedAtMs: analysis.capturedAtMs,
    id: analysis.screenshotId,
    payload: {
      appName: analysis.appName,
      windowTitle: analysis.windowTitle,
      summary: analysis.summary,
      reason: analysis.screenshotKind,
    },
  };
}

export function mergeEntries(
  analyses: DailyObservationAnalysis[] | undefined,
  liveEntries: ActivityCaptureEntry[],
) {
  return liveEntries.reduce(
    (current, entry) => upsertEntry(current, entry),
    (analyses ?? []).map(snapshotAnalysisToEntry),
  );
}

export function emptyStatus(): ActivityCaptureStatus {
  return {
    isRunning: false,
    lastStateChangedAtMs: null,
    currentObservation: null,
    lastObservationEvent: null,
    lastError: null,
    lastObservationAnalysis: null,
    lastObservationAnalysisError: null,
    config: {
      pollIntervalMs: 0,
      entryDwellMs: 0,
      typingSettleMs: 0,
      longTypingCheckpointMs: 0,
      refreshIntervalMs: 0,
    },
    analyzeScreenshots: false,
    screenshotsToday: 0,
    screenshotsThisHour: 0,
    storageUsedMb: 0,
  };
}

export function updateStatus(
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

  if (
    payload.type === "activityObservationStarted" ||
    payload.type === "activityObservationCheckpointed"
  ) {
    return {
      ...next,
      lastObservationEvent: payload.event,
    };
  }

  if (payload.type === "activityObservationEnded") {
    return {
      ...next,
      currentObservation: null,
      lastObservationEvent: payload.event,
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

  if (payload.type === "activityObservationAnalysisReady") {
    return {
      ...next,
      lastObservationAnalysis: payload.analysis,
      lastObservationAnalysisError: null,
    };
  }

  if (payload.type === "activityObservationAnalysisError") {
    return {
      ...next,
      lastObservationAnalysisError: payload.error,
    };
  }

  return next;
}

export function readableReason(reason: string) {
  return reason.split("_").join(" ");
}

export function appContext(appName: string, windowTitle: string | null) {
  return windowTitle ? `${appName} · ${windowTitle}` : appName;
}

export function formatSignalTime(value: number | null, tz?: string) {
  if (!value) {
    return "—";
  }
  return formatTime(value, tz);
}

export function toError(error: unknown) {
  if (error instanceof Error) {
    return error;
  }

  return new Error(String(error));
}

export function renderRuntimeError(error: ActivityCaptureRuntimeError) {
  return `${readableReason(error.kind)}: ${error.message}`;
}
