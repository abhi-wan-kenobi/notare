import { tool } from "ai";
import { z } from "zod";

import {
  getMeeting,
  getMeetingTranscript,
  getRecurringMeetingHistory,
  listMeetings,
  type MeetingPage,
} from "@hypr/plugin-db";

import { resolveAllowedSessionIds } from "./scope-resolver";
import type { ToolDependencies } from "./types";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";

const MAX_SCOPE_PAGE_WALK = 10;

/**
 * Pages through the unfiltered meeting list until `limit` in-scope meetings
 * are collected (or the source is exhausted). `pagination.next_offset` stays a
 * raw-list offset so the model can keep paging; `total` is unknown under a
 * filter and reported as null.
 */
export async function collectMeetingsInScope(
  fetchPage: (params: {
    offset?: number;
    limit?: number;
  }) => Promise<MeetingPage>,
  params: { offset?: number; limit?: number },
  allowedSessionIds: readonly string[],
): Promise<MeetingPage> {
  const allowed = new Set(allowedSessionIds);
  const limit = params.limit ?? 20;
  const startOffset = params.offset ?? 0;
  const collected: MeetingPage["meetings"] = [];
  let offset = startOffset;
  let nextOffset: number | null = startOffset;

  for (let walk = 0; walk < MAX_SCOPE_PAGE_WALK; walk += 1) {
    if (allowed.size === 0) {
      nextOffset = null;
      break;
    }
    const page = await fetchPage({ ...params, offset, limit });
    for (const meeting of page.meetings) {
      if (allowed.has(meeting.id)) {
        collected.push(meeting);
      }
    }
    nextOffset = page.pagination.next_offset;
    if (collected.length >= limit || nextOffset === null) {
      break;
    }
    offset = nextOffset;
  }

  const meetings = collected.slice(0, limit);
  return {
    meetings,
    pagination: {
      offset: startOffset,
      limit,
      returned: meetings.length,
      total: null,
      next_offset: nextOffset,
    },
  };
}

const listLimitSchema = z
  .number()
  .int()
  .min(1)
  .max(200)
  .optional()
  .describe("Maximum results; defaults to 20 and is capped at 200");

const offsetSchema = z
  .number()
  .int()
  .min(0)
  .optional()
  .describe("Number of results to skip; defaults to 0");

const historyLimitSchema = z
  .number()
  .int()
  .min(1)
  .max(200)
  .optional()
  .describe("Maximum meetings; defaults to 20 and is capped at 200");

const historyOffsetSchema = z
  .number()
  .int()
  .min(0)
  .optional()
  .describe("Number of meetings to skip; defaults to 0");

export const buildListMeetingsTool = (
  deps?: ToolDependencies,
  scopeFilterOverride?: ToolScopeFilter | (() => ToolScopeFilter | undefined),
) =>
  tool({
    description:
      "List recent Notare meetings with pagination metadata. Use query to narrow by title or meeting id, then pass next_offset as offset to continue.",
    inputSchema: z.object({
      query: z
        .string()
        .optional()
        .describe("Case-insensitive title or meeting id substring"),
      series_id: z.string().optional().describe("Exact recurring series id"),
      limit: listLimitSchema,
      offset: offsetSchema,
    }),
    execute: async (params) => {
      const activeFilter =
        typeof scopeFilterOverride === "function"
          ? scopeFilterOverride()
          : (scopeFilterOverride ??
            (typeof deps?.scopeFilter === "function"
              ? deps.scopeFilter()
              : deps?.scopeFilter));

      const allowedSessionIds = await resolveAllowedSessionIds(
        activeFilter,
        deps,
      );
      if (allowedSessionIds === null) {
        return listMeetings(params);
      }

      return collectMeetingsInScope(
        (page) => listMeetings({ ...params, ...page }),
        params,
        allowedSessionIds,
      );
    },
  });

export const buildGetMeetingTool = () =>
  tool({
    description:
      "Get one Notare meeting with its canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for transcript words.",
    inputSchema: z.object({
      meeting_id: z.string().describe("Notare meeting id"),
    }),
    execute: getMeeting,
  });

export const buildGetMeetingTranscriptTool = () =>
  tool({
    description:
      "Get a bounded page of transcript words and readable text for an Notare meeting. Pass pagination.next_offset as offset to continue.",
    inputSchema: z.object({
      meeting_id: z.string().describe("Notare meeting id"),
      offset: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe("Word offset; defaults to 0"),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .describe("Maximum words; defaults to 200 and is capped at 500"),
    }),
    execute: getMeetingTranscript,
  });

export const buildGetRecurringMeetingHistoryTool = () =>
  tool({
    description:
      "List meetings in the same recurring series as the supplied meeting, newest first, with pagination metadata.",
    inputSchema: z.object({
      meeting_id: z
        .string()
        .describe("A meeting id used to resolve its recurring series"),
      limit: historyLimitSchema,
      offset: historyOffsetSchema,
    }),
    execute: getRecurringMeetingHistory,
  });
