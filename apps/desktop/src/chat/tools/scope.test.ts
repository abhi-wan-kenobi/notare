import type { ToolSet } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { buildListMeetingsTool } from "./meetings";
import {
  buildFindRelatedMeetingsTool,
  buildSearchMeetingContentTool,
} from "./note-files";
import {
  applyScopeFilterToTools,
  CROSS_MEETING_TOOL_NAMES,
} from "./scope-tools";
import { buildSearchMeetingsTool } from "./search-meetings";
import type { ToolDependencies } from "./types";

import type { ToolScopeFilter } from "~/chat/state/chat-scope";

const mocks = vi.hoisted(() => ({
  loadActiveSessionIds: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  listMeetings: vi.fn(),
}));

vi.mock("~/session/content-queries", () => ({
  loadActiveSessionIds: mocks.loadActiveSessionIds,
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));
vi.mock(import("@hypr/plugin-db"), async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    listMeetings: mocks.listMeetings,
  };
});

async function executeTool<TInput, TOutput>(
  toolInstance: unknown,
  input: TInput,
): Promise<TOutput> {
  if (
    toolInstance &&
    typeof toolInstance === "object" &&
    "execute" in toolInstance &&
    typeof toolInstance.execute === "function"
  ) {
    return toolInstance.execute(input);
  }
  throw new Error("Tool has no execute function");
}

describe("tool layer scope filter enforcement", () => {
  const dummyDependencies = (): ToolDependencies => ({
    search: vi.fn(),
    getContactSearchResults: vi.fn(),
    getCalendarEventSearchResults: vi.fn(),
    getSessionId: vi.fn().mockReturnValue("s1"),
    getEnhancedNoteId: vi.fn(),
    openEditTab: vi.fn(),
    getAuthHeaders: vi.fn(),
    resolveSessionIdsForHuman: vi.fn().mockImplementation(async (humanId) => {
      if (humanId === "human-alice") return ["s1", "s2"];
      return [];
    }),
    resolveSessionIdsForOrg: vi.fn().mockImplementation(async (orgId) => {
      if (orgId === "org-acme") return ["s2", "s3"];
      return [];
    }),
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadActiveSessionIds.mockResolvedValue(["s1", "s2", "s3"]);
    mocks.loadSessionContentSnapshot.mockImplementation(
      async (sessionId: string) => ({
        sessionId,
        title: `Meeting ${sessionId}`,
        createdAt: "2026-11-01T10:00:00Z",
        event: null,
        eventId: null,
        rawMarkdown: `Content of ${sessionId}`,
        enhancedNotes: [],
        transcripts: [],
        participants: [],
      }),
    );
  });

  describe("search_meetings scope filtering", () => {
    it("filters search results by session_ids", async () => {
      const deps = dummyDependencies();
      vi.mocked(deps.search).mockResolvedValue([
        {
          score: 0.9,
          document: {
            id: "s1",
            type: "session",
            title: "S1",
            content: "x",
            created_at: 1,
          },
        },
        {
          score: 0.8,
          document: {
            id: "s2",
            type: "session",
            title: "S2",
            content: "x",
            created_at: 2,
          },
        },
        {
          score: 0.7,
          document: {
            id: "s3",
            type: "session",
            title: "S3",
            content: "x",
            created_at: 3,
          },
        },
      ]);

      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["s2"],
      };

      const tool = buildSearchMeetingsTool(deps, scopeFilter);
      const result = await executeTool<
        { query: string },
        { results: Array<{ id: string }> }
      >(tool, { query: "test" });

      expect(result.results.map((r) => r.id)).toEqual(["s2"]);
    });

    it("filters search results by human_id", async () => {
      const deps = dummyDependencies();
      vi.mocked(deps.search).mockResolvedValue([
        {
          score: 0.9,
          document: {
            id: "s1",
            type: "session",
            title: "S1",
            content: "x",
            created_at: 1,
          },
        },
        {
          score: 0.8,
          document: {
            id: "s2",
            type: "session",
            title: "S2",
            content: "x",
            created_at: 2,
          },
        },
        {
          score: 0.7,
          document: {
            id: "s3",
            type: "session",
            title: "S3",
            content: "x",
            created_at: 3,
          },
        },
      ]);

      const scopeFilter: ToolScopeFilter = {
        kind: "human_id",
        humanId: "human-alice",
      };

      const tool = buildSearchMeetingsTool(deps, scopeFilter);
      const result = await executeTool<
        { query: string },
        { results: Array<{ id: string }> }
      >(tool, { query: "test" });

      expect(result.results.map((r) => r.id)).toEqual(["s1", "s2"]);
    });

    it("filters search results by org_id", async () => {
      const deps = dummyDependencies();
      vi.mocked(deps.search).mockResolvedValue([
        {
          score: 0.9,
          document: {
            id: "s1",
            type: "session",
            title: "S1",
            content: "x",
            created_at: 1,
          },
        },
        {
          score: 0.8,
          document: {
            id: "s2",
            type: "session",
            title: "S2",
            content: "x",
            created_at: 2,
          },
        },
        {
          score: 0.7,
          document: {
            id: "s3",
            type: "session",
            title: "S3",
            content: "x",
            created_at: 3,
          },
        },
      ]);

      const scopeFilter: ToolScopeFilter = {
        kind: "org_id",
        orgId: "org-acme",
      };

      const tool = buildSearchMeetingsTool(deps, scopeFilter);
      const result = await executeTool<
        { query: string },
        { results: Array<{ id: string }> }
      >(tool, { query: "test" });

      expect(result.results.map((r) => r.id)).toEqual(["s2", "s3"]);
    });
  });

  describe("search_meeting_content scope filtering", () => {
    it("prevents model from escaping session_ids scope filter", async () => {
      const deps = dummyDependencies();
      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["s1"],
      };

      const tool = buildSearchMeetingContentTool(deps, scopeFilter);
      // Model tries to escape scope by requesting meeting_ids: ["s2", "s3"]
      const result = await executeTool<
        { query: string; meeting_ids?: string[] },
        { results: Array<{ meeting_id: string }> }
      >(tool, {
        query: "Content",
        meeting_ids: ["s2", "s3"],
      });

      expect(result.results).toEqual([]);
    });

    it("includes stable source_id in search_meeting_content results and snippets", async () => {
      const deps = dummyDependencies();
      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["s1"],
      };

      const tool = buildSearchMeetingContentTool(deps, scopeFilter);
      const result = await executeTool<
        { query: string },
        {
          results: Array<{
            meeting_id: string;
            source_id?: string;
            snippets: Array<{ source_id?: string }>;
          }>;
        }
      >(tool, { query: "Content" });

      expect(result.results).toHaveLength(1);
      expect(result.results[0]?.meeting_id).toBe("s1");
      expect(result.results[0]?.source_id).toBe("[[src:s1@0]]");
      expect(result.results[0]?.snippets[0]?.source_id).toBe("[[src:s1@0]]");
    });
  });

  describe("find_related_meetings scope filtering", () => {
    it("restricts related meetings to within the allowed scope", async () => {
      const deps = dummyDependencies();
      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["s2"],
      };

      const tool = buildFindRelatedMeetingsTool(deps, scopeFilter);
      const result = await executeTool<
        { meeting_id: string },
        { results: Array<{ meeting_id: string }> }
      >(tool, { meeting_id: "s1" });

      // s3 is excluded because it is not in the scope
      for (const item of result.results) {
        expect(item.meeting_id).toBe("s2");
      }
    });
  });

  describe("list_meetings scope filtering", () => {
    it("keeps paging the raw list until the requested in-scope count is met", async () => {
      const deps = dummyDependencies();
      // Raw list: page 1 has one in-scope meeting, page 2 has the other two.
      mocks.listMeetings.mockImplementation(
        async ({ offset }: { offset?: number }) =>
          (offset ?? 0) === 0
            ? {
                meetings: [{ id: "x1" }, { id: "s1" }],
                pagination: {
                  offset: 0,
                  limit: 2,
                  returned: 2,
                  total: 4,
                  next_offset: 2,
                },
              }
            : {
                meetings: [{ id: "s3" }, { id: "x2" }],
                pagination: {
                  offset: 2,
                  limit: 2,
                  returned: 2,
                  total: 4,
                  next_offset: null,
                },
              },
      );

      const scopeFilter: ToolScopeFilter = {
        kind: "session_ids",
        sessionIds: ["s1", "s3"],
      };

      const tool = buildListMeetingsTool(deps, scopeFilter);
      const result = await executeTool<
        Record<string, unknown>,
        {
          meetings: Array<{ id: string }>;
          pagination: { returned: number; next_offset: number | null };
        }
      >(tool, { limit: 2 });

      expect(result.meetings.map((m) => m.id)).toEqual(["s1", "s3"]);
      expect(result.pagination.returned).toBe(2);
      expect(result.pagination.next_offset).toBeNull();
      expect(mocks.listMeetings).toHaveBeenCalledTimes(2);
    });

    it("returns nothing without querying when the scope is empty", async () => {
      const deps = dummyDependencies();
      const tool = buildListMeetingsTool(deps, {
        kind: "session_ids",
        sessionIds: [],
      });
      const result = await executeTool<
        Record<string, unknown>,
        { meetings: unknown[] }
      >(tool, {});
      expect(result.meetings).toEqual([]);
      expect(mocks.listMeetings).not.toHaveBeenCalled();
    });
  });

  describe("id-addressed tools cannot read outside the scope", () => {
    it("refuses get_meeting / get_meeting_transcript for out-of-scope ids", async () => {
      const deps = dummyDependencies();
      const execute = vi.fn(async () => ({ id: "x9" }));
      const rawTools = {
        get_meeting: { description: "", inputSchema: {}, execute },
        get_meeting_transcript: { description: "", inputSchema: {}, execute },
      } as unknown as ToolSet;
      const scoped = applyScopeFilterToTools(
        rawTools,
        { kind: "session_ids", sessionIds: ["s1"] },
        false,
        deps,
      );

      for (const name of ["get_meeting", "get_meeting_transcript"] as const) {
        const denied = await executeTool<
          { meeting_id: string },
          { error?: string }
        >(scoped[name], { meeting_id: "x9" });
        expect(denied.error).toMatch(/outside the current chat scope/);
        const allowed = await executeTool<{ meeting_id: string }, unknown>(
          scoped[name],
          { meeting_id: "s1" },
        );
        expect(allowed).toEqual({ id: "x9" });
      }
      expect(execute).toHaveBeenCalledTimes(2);
    });

    it("filters get_recurring_meeting_history results to the scope", async () => {
      const deps = dummyDependencies();
      const rawTools = {
        get_recurring_meeting_history: {
          description: "",
          inputSchema: {},
          execute: vi.fn(async () => ({
            meetings: [{ id: "s1" }, { id: "x1" }],
            pagination: {
              offset: 0,
              limit: 20,
              returned: 2,
              total: 2,
              next_offset: null,
            },
          })),
        },
      } as unknown as ToolSet;
      const scoped = applyScopeFilterToTools(
        rawTools,
        { kind: "session_ids", sessionIds: ["s1"] },
        false,
        deps,
      );
      const result = await executeTool<
        { meeting_id: string },
        { meetings: Array<{ id: string }>; pagination: { returned: number } }
      >(scoped.get_recurring_meeting_history, { meeting_id: "s1" });
      expect(result.meetings.map((m) => m.id)).toEqual(["s1"]);
      expect(result.pagination.returned).toBe(1);
    });
  });

  describe("applyScopeFilterToTools - This meeting cross-meeting tool removal", () => {
    it("removes cross-meeting tools when isThisMeeting is true", () => {
      const deps = dummyDependencies();
      const rawTools = {
        list_meetings: buildListMeetingsTool(deps),
        search_meetings: buildSearchMeetingsTool(deps),
        find_related_meetings: buildFindRelatedMeetingsTool(deps),
        get_recurring_meeting_history: buildListMeetingsTool(deps),
        search_meeting_content: buildSearchMeetingContentTool(deps),
      };

      const scoped = applyScopeFilterToTools(
        rawTools,
        { kind: "session_ids", sessionIds: ["s1"] },
        true, // isThisMeeting
        deps,
      );

      for (const name of CROSS_MEETING_TOOL_NAMES) {
        expect(scoped).not.toHaveProperty(name);
      }
      expect(scoped).toHaveProperty("search_meeting_content");
    });
  });
});
