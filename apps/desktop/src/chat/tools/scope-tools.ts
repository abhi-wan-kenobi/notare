import type { ToolSet } from "ai";

import { resolveAllowedSessionIds } from "./scope-resolver";
import type { ToolDependencies } from "./types";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";

export const CROSS_MEETING_TOOL_NAMES = [
  "search_meetings",
  "list_meetings",
  "find_related_meetings",
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

  // Wrap list_meetings (if not deleted)
  if (nextTools.list_meetings && typeof nextTools.list_meetings === "object") {
    const original = nextTools.list_meetings as {
      execute?: (params: {
        query?: string;
        limit?: number;
        offset?: number;
      }) => Promise<{ meetings?: Array<{ id: string }>; total?: number }>;
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
          const page = await originalExecute(params);
          const allowedSessionIds = await resolveAllowedSessionIds(
            scopeFilter,
            deps,
          );
          if (allowedSessionIds !== null && page?.meetings) {
            const filtered = page.meetings.filter((m) =>
              allowedSessionIds.includes(m.id),
            );
            return {
              ...page,
              meetings: filtered,
              total: filtered.length,
              next_offset: null,
            };
          }
          return page;
        },
      };
    }
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
