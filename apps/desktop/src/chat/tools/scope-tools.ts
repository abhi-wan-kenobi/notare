import type { ToolSet } from "ai";

import type { MeetingPage } from "@hypr/plugin-db";

import { collectMeetingsInScope } from "./meetings";
import { resolveAllowedSessionIds } from "./scope-resolver";
import type { ToolDependencies } from "./types";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";

export const CROSS_MEETING_TOOL_NAMES = [
  "search_meetings",
  "list_meetings",
  "find_related_meetings",
  "get_recurring_meeting_history",
] as const;

const ID_GUARDED_TOOL_NAMES = [
  "get_meeting",
  "get_meeting_transcript",
  "get_recurring_meeting_history",
] as const;

export function applyScopeFilterToTools(
  tools: ToolSet,
  scopeFilter: ToolScopeFilter | undefined,
  isThisMeeting: boolean,
  deps?: ToolDependencies,
): ToolSet {
  const nextTools: ToolSet = { ...tools };

  if (isThisMeeting) {
    for (const name of CROSS_MEETING_TOOL_NAMES) {
      delete nextTools[name];
    }
  }

  // If no filter or empty filter, return as is (with cross-meeting tools removed if isThisMeeting)
  if (!scopeFilter || scopeFilter.kind === "none") {
    return nextTools;
  }

  // Wrap search_meeting_content
  if (
    nextTools.search_meeting_content &&
    typeof nextTools.search_meeting_content === "object"
  ) {
    const original = nextTools.search_meeting_content as {
      execute?: (params: {
        query: string;
        meeting_ids?: string[];
        limit?: number;
      }) => Promise<unknown>;
    };
    if (typeof original.execute === "function") {
      const originalExecute = original.execute.bind(
        nextTools.search_meeting_content,
      );
      nextTools.search_meeting_content = {
        ...nextTools.search_meeting_content,
        execute: async (params: {
          query: string;
          meeting_ids?: string[];
          limit?: number;
        }) => {
          const allowedSessionIds = await resolveAllowedSessionIds(
            scopeFilter,
            deps,
          );
          let sessionIds = params.meeting_ids;
          if (allowedSessionIds !== null) {
            if (sessionIds && sessionIds.length > 0) {
              sessionIds = sessionIds.filter((id) =>
                allowedSessionIds.includes(id),
              );
            } else {
              sessionIds = allowedSessionIds;
            }
            if (sessionIds.length === 0) {
              return {
                query: params.query,
                scanned: 0,
                results: [],
              };
            }
          }

          return originalExecute({
            ...params,
            meeting_ids: sessionIds,
          });
        },
      };
    }
  }

  // Wrap search_meetings (if not deleted)
  if (
    nextTools.search_meetings &&
    typeof nextTools.search_meetings === "object"
  ) {
    const original = nextTools.search_meetings as {
      execute?: (params: {
        query?: string;
        limit?: number;
      }) => Promise<{ results?: Array<{ id: string }> }>;
    };
    if (typeof original.execute === "function") {
      const originalExecute = original.execute.bind(nextTools.search_meetings);
      nextTools.search_meetings = {
        ...nextTools.search_meetings,
        execute: async (params: { query?: string; limit?: number }) => {
          const result = await originalExecute(params);
          const allowedSessionIds = await resolveAllowedSessionIds(
            scopeFilter,
            deps,
          );
          if (allowedSessionIds !== null && result?.results) {
            return {
              ...result,
              results: result.results.filter((item) =>
                allowedSessionIds.includes(item.id),
              ),
            };
          }
          return result;
        },
      };
    }
  }

  // Wrap list_meetings (if not deleted): page through the raw list until the
  // requested number of in-scope meetings is found, keeping next_offset valid.
  if (nextTools.list_meetings && typeof nextTools.list_meetings === "object") {
    const original = nextTools.list_meetings as {
      execute?: (params: {
        query?: string;
        limit?: number;
        offset?: number;
      }) => Promise<MeetingPage>;
    };
    if (typeof original.execute === "function") {
      const originalExecute = original.execute.bind(nextTools.list_meetings);
      nextTools.list_meetings = {
        ...nextTools.list_meetings,
        execute: async (params: {
          query?: string;
          limit?: number;
          offset?: number;
        }) => {
          const allowedSessionIds = await resolveAllowedSessionIds(
            scopeFilter,
            deps,
          );
          if (allowedSessionIds === null) {
            return originalExecute(params);
          }
          return collectMeetingsInScope(
            (page) => originalExecute({ ...params, ...page }),
            params,
            allowedSessionIds,
          );
        },
      };
    }
  }

  // Tools addressed by meeting_id must not read meetings outside the scope,
  // even if the model learned an id from earlier conversation context.
  for (const name of ID_GUARDED_TOOL_NAMES) {
    const current = nextTools[name];
    if (!current || typeof current !== "object") {
      continue;
    }
    const original = current as {
      execute?: (params: { meeting_id: string }) => Promise<unknown>;
    };
    if (typeof original.execute !== "function") {
      continue;
    }
    const originalExecute = original.execute.bind(current);
    nextTools[name] = {
      ...current,
      execute: async (params: { meeting_id: string }) => {
        const allowedSessionIds = await resolveAllowedSessionIds(
          scopeFilter,
          deps,
        );
        if (
          allowedSessionIds !== null &&
          !allowedSessionIds.includes(params.meeting_id)
        ) {
          return {
            error: `Meeting ${params.meeting_id} is outside the current chat scope. Change the scope selector to access it.`,
          };
        }
        const result = await originalExecute(params);
        if (
          name === "get_recurring_meeting_history" &&
          allowedSessionIds !== null &&
          result &&
          typeof result === "object" &&
          Array.isArray((result as MeetingPage).meetings)
        ) {
          const page = result as MeetingPage;
          const meetings = page.meetings.filter((m) =>
            allowedSessionIds.includes(m.id),
          );
          return {
            ...page,
            meetings,
            pagination: {
              ...page.pagination,
              returned: meetings.length,
              total: null,
            },
          };
        }
        return result;
      },
    };
  }

  // Wrap find_related_meetings (if not deleted)
  if (
    nextTools.find_related_meetings &&
    typeof nextTools.find_related_meetings === "object"
  ) {
    const original = nextTools.find_related_meetings as {
      execute?: (params: { meeting_id?: string; limit?: number }) => Promise<{
        results?: Array<{ meeting_id?: string; id?: string }>;
      }>;
    };
    if (typeof original.execute === "function") {
      const originalExecute = original.execute.bind(
        nextTools.find_related_meetings,
      );
      nextTools.find_related_meetings = {
        ...nextTools.find_related_meetings,
        execute: async (params: { meeting_id?: string; limit?: number }) => {
          const res = await originalExecute(params);
          const allowedSessionIds = await resolveAllowedSessionIds(
            scopeFilter,
            deps,
          );
          if (allowedSessionIds !== null && res?.results) {
            return {
              ...res,
              results: res.results.filter((m) => {
                const id = m.meeting_id ?? m.id;
                return id && allowedSessionIds.includes(id);
              }),
            };
          }
          return res;
        },
      };
    }
  }

  return nextTools;
}
