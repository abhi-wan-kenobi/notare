import type { LanguageModel } from "ai";
import { useEffect, useRef } from "react";
import { useScheduleTaskRun, useSetTask } from "tinytick/ui-react";

import { generateBriefForEvent } from "./generator";
import { loadBriefedEventIds } from "./storage";
import type { BriefCandidateEvent } from "./types";

import {
  excludeHiddenProviderEventsSql,
  getHiddenCalendarProviders,
  hiddenProvidersParam,
} from "~/calendar/hidden-providers";
import { parseEventParticipants } from "~/calendar/queries";
import { liveQueryClient } from "~/db";
import { useTaskModel } from "~/services/llm-router";
import { useConfigValue } from "~/shared/config";

export const PRE_MEETING_BRIEFS_TASK_ID = "preMeetingBriefs";
export const PRE_MEETING_BRIEFS_INTERVAL = 5 * 60 * 1000; // 5 minutes
export const DEFAULT_LEAD_TIME_HOURS = 12;

export function selectCandidateEvents(
  events: BriefCandidateEvent[],
  now: number,
  leadTimeHours: number,
  alreadyBriefedEventIds: Set<string>,
): BriefCandidateEvent[] {
  const leadTimeMs = leadTimeHours * 60 * 60 * 1000;
  const maxTime = now + leadTimeMs;
  const seenIds = new Set<string>();
  const candidates: BriefCandidateEvent[] = [];

  for (const event of events) {
    if (!event.id || seenIds.has(event.id)) {
      continue;
    }

    if (event.is_all_day) {
      continue;
    }

    if (alreadyBriefedEventIds.has(event.id)) {
      continue;
    }

    if (!event.started_at) {
      continue;
    }

    let startTime: number;
    try {
      startTime = new Date(event.started_at).getTime();
    } catch {
      continue;
    }

    if (Number.isNaN(startTime)) {
      continue;
    }

    if (event.ended_at) {
      let endTime: number;
      try {
        endTime = new Date(event.ended_at).getTime();
      } catch {
        continue;
      }
      if (Number.isNaN(endTime)) {
        continue;
      }
    }

    if (startTime < now || startTime > maxTime) {
      continue;
    }

    const participants = parseEventParticipants(
      event.participants_json && event.participants_json.trim() !== ""
        ? event.participants_json
        : undefined,
    ).filter((p) => p.is_current_user !== true);

    if (participants.length < 1) {
      continue;
    }

    seenIds.add(event.id);
    candidates.push(event);
  }

  return candidates;
}

export async function fetchCalendarEvents(): Promise<BriefCandidateEvent[]> {
  const hiddenProviders = await getHiddenCalendarProviders();
  return liveQueryClient.execute<BriefCandidateEvent>(
    `
    SELECT
      id,
      title,
      started_at,
      ended_at,
      location,
      meeting_link,
      description,
      calendar_id,
      recurrence_series_id,
      participants_json,
      is_all_day
    FROM events
    WHERE deleted_at IS NULL
      AND is_all_day = 0
      AND started_at <> ''
      AND ${excludeHiddenProviderEventsSql("events")}
    ORDER BY started_at, id
  `,
    [hiddenProvidersParam(hiddenProviders)],
  );
}

export async function checkAndGenerateBriefs(params: {
  enabled: boolean;
  leadTimeHours?: number;
  model: LanguageModel | null;
  modelName?: string;
  now?: number;
  signal?: AbortSignal;
}): Promise<number> {
  if (!params.enabled || !params.model) {
    return 0;
  }

  const now = params.now ?? Date.now();
  const leadTimeHours = params.leadTimeHours ?? DEFAULT_LEAD_TIME_HOURS;

  const [briefedIds, allEvents] = await Promise.all([
    loadBriefedEventIds(),
    fetchCalendarEvents(),
  ]);

  const candidates = selectCandidateEvents(
    allEvents,
    now,
    leadTimeHours,
    briefedIds,
  );

  let generatedCount = 0;
  for (const candidate of candidates) {
    if (params.signal?.aborted) break;

    const brief = await generateBriefForEvent(
      candidate,
      params.model,
      params.modelName ?? "notes-scope",
      { signal: params.signal },
    );

    if (brief) {
      generatedCount += 1;
    }
  }

  return generatedCount;
}

export function useBriefsScheduler() {
  const enabledConfig = useConfigValue("pre_meeting_briefs_enabled");
  const leadTimeHoursConfig = useConfigValue(
    "pre_meeting_briefs_lead_time_hours",
  );
  const { model, resolution } = useTaskModel("brief");

  const modelAvailable = resolution.status === "ok" && Boolean(model);
  const enabled = (enabledConfig ?? true) && modelAvailable;
  const leadTimeHours = leadTimeHoursConfig ?? DEFAULT_LEAD_TIME_HOURS;
  const modelName =
    resolution.status === "ok"
      ? `${resolution.providerId}/${resolution.modelId}`
      : "notes-scope";

  const abortControllerRef = useRef<AbortController | null>(null);

  useSetTask(PRE_MEETING_BRIEFS_TASK_ID, async () => {
    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      await checkAndGenerateBriefs({
        enabled,
        leadTimeHours,
        model,
        modelName,
        signal: controller.signal,
      });
    } catch (err) {
      console.error("[briefs] scheduler task failed", err);
    } finally {
      if (abortControllerRef.current === controller) {
        abortControllerRef.current = null;
      }
    }
  }, [enabled, leadTimeHours, model, modelName]);

  useScheduleTaskRun(PRE_MEETING_BRIEFS_TASK_ID, undefined, 0, {
    repeatDelay: PRE_MEETING_BRIEFS_INTERVAL,
  });

  useEffect(() => {
    return () => {
      abortControllerRef.current?.abort();
    };
  }, []);
}
