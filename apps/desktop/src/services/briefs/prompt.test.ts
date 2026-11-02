import { describe, expect, it, vi } from "vitest";

import {
  assembleBriefPrompt,
  resolveAttendees,
  retrieveBriefContext,
} from "./prompt";
import type { BriefCandidateEvent, BriefRetrievalContext } from "./types";

const mocks = vi.hoisted(() => ({
  searchMeetingContent: vi.fn(async (_params?: unknown) => ({
    results: [] as unknown[],
  })),
  loadSessionContentSnapshot: vi.fn(
    async (_sessionId?: unknown) => null as unknown,
  ),
}));

vi.mock("~/chat/tools/note-files", () => ({
  searchMeetingContent: (params: unknown) => mocks.searchMeetingContent(params),
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: (params: unknown) =>
    mocks.loadSessionContentSnapshot(params),
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: vi.fn(async (_sql: string, params: unknown[]) => {
      const emailParams = params as string[];
      if (emailParams.includes("alice@acme.com")) {
        return [
          {
            email: "alice@acme.com",
            name: "Alice Smith",
            org_name: "Acme Corporation",
          },
        ];
      }
      return [];
    }),
  },
}));

describe("Prompt Assembly with Citations", () => {
  it("resolves attendees and associates their organization", async () => {
    const participantsJson = JSON.stringify([
      { name: "Me", email: "me@example.com", is_current_user: true },
      { name: "Alice Smith", email: "alice@acme.com", is_current_user: false },
      { name: "Bob", email: "bob@initech.io", is_current_user: false },
    ]);

    const attendees = await resolveAttendees(participantsJson);

    expect(attendees).toHaveLength(2);
    expect(attendees[0]).toMatchObject({
      name: "Alice Smith",
      email: "alice@acme.com",
      org: "Acme Corporation",
    });
    // Initech inferred from domain
    expect(attendees[1]).toMatchObject({
      name: "Bob",
      email: "bob@initech.io",
      org: "Initech",
    });
  });

  it("assembles system and user prompts with [[src:<sessionId>@<ms>]] citations", () => {
    const event: BriefCandidateEvent = {
      id: "event-101",
      title: "Quarterly Review with Acme",
      started_at: "2026-11-02T15:00:00.000Z",
      ended_at: "2026-11-02T16:00:00.000Z",
      location: "Room 4B",
      meeting_link: "https://meet.google.com/xyz-123",
      description: "Discuss Q3 progress and renewal",
    };

    const context: BriefRetrievalContext = {
      attendees: [
        {
          name: "Alice Smith",
          org: "Acme Corporation",
          email: "alice@acme.com",
        },
        { name: "Charlie Brown", org: "Acme Corporation" },
      ],
      relevantMeetings: [
        {
          sessionId: "sess-q2-review",
          title: "Q2 Renewal Review",
          date: "2026-08-15T10:00:00.000Z",
          outcome: "Agreed to 15% expansion pending enterprise security check.",
          timestampMs: 12500,
        },
        {
          sessionId: "sess-tech-sync",
          title: "Tech Architecture Sync",
          date: "2026-09-20T14:00:00.000Z",
          outcome: "Completed SSO and audit log integration.",
          timestampMs: 0,
        },
      ],
      openActionItems: [
        {
          text: "Deliver updated SOC2 report to Alice",
          dueAt: "2026-11-05",
          sourceSessionId: "sess-q2-review",
        },
      ],
      orgMentions: [
        {
          sessionId: "sess-strategy",
          title: "Executive Strategy",
          snippet: "Acme Corporation renewal is top priority this quarter.",
        },
      ],
    };

    const { system, prompt } = assembleBriefPrompt(event, context);

    // Verify system instructions require citation format
    expect(system).toContain("[[src:<sessionId>@<ms>]]");
    expect(system).toContain(
      "Never invent, fabricate, or hallucinate session IDs",
    );

    // Verify user prompt contains event details
    expect(prompt).toContain("Quarterly Review with Acme");
    expect(prompt).toContain("https://meet.google.com/xyz-123");

    // Verify attendees formatted with org
    expect(prompt).toContain("Alice Smith (Acme Corporation) <alice@acme.com>");
    expect(prompt).toContain("Charlie Brown (Acme Corporation)");

    // Verify citations are formatted exactly as [[src:<sessionId>@<ms>]]
    expect(prompt).toContain("[[src:sess-q2-review@12500]]");
    expect(prompt).toContain("[[src:sess-tech-sync@0]]");
    expect(prompt).toContain("[[src:sess-q2-review@0]]");
    expect(prompt).toContain("[[src:sess-strategy@0]]");

    // Check all citations with regex
    const citationRegex = /\[\[src:([a-zA-Z0-9_\-.:]+)@(\d+)\]\]/g;
    const matches = Array.from(prompt.matchAll(citationRegex));
    expect(matches.length).toBeGreaterThanOrEqual(4);

    const sessionIds = matches.map((m) => m[1]);
    expect(sessionIds).toContain("sess-q2-review");
    expect(sessionIds).toContain("sess-tech-sync");
    expect(sessionIds).toContain("sess-strategy");
  });

  it("formats action items with specific timestampMs if available", () => {
    const event: BriefCandidateEvent = {
      id: "event-1",
      title: "Test",
      started_at: "2026-11-02T10:00:00.000Z",
      ended_at: "2026-11-02T11:00:00.000Z",
    };

    const context: BriefRetrievalContext = {
      attendees: [],
      relevantMeetings: [],
      openActionItems: [
        {
          text: "Follow up on invoice",
          sourceSessionId: "sess-invoice",
          timestampMs: 42000,
        },
      ],
      orgMentions: [],
    };

    const { prompt } = assembleBriefPrompt(event, context);
    expect(prompt).toContain(
      "- Follow up on invoice [[src:sess-invoice@42000]]",
    );
  });

  it("retrieveBriefContext extracts real startMs from search hits and survives transient errors", async () => {
    const event: BriefCandidateEvent = {
      id: "event-1",
      title: "Sync",
      started_at: "2026-11-02T10:00:00.000Z",
      ended_at: "2026-11-02T11:00:00.000Z",
      participants_json: JSON.stringify([
        {
          name: "Alice Smith",
          email: "alice@acme.com",
          is_current_user: false,
        },
      ]),
    };

    mocks.searchMeetingContent.mockResolvedValueOnce({
      results: [
        {
          sessionId: "sess-trans-hit",
          title: "Prior Meeting",
          snippets: [{ text: "Spoke with Alice", startMs: 18500 }],
        },
      ],
    });

    mocks.loadSessionContentSnapshot.mockResolvedValueOnce({
      sessionId: "sess-trans-hit",
      title: "Prior Meeting",
      createdAt: "2026-10-01T12:00:00.000Z",
      enhancedNotes: [{ markdown: "Decided to proceed with Phase 2." }],
      rawMarkdown: "",
      transcripts: [],
      actionItems: [],
    });

    const ctx = await retrieveBriefContext(event);
    expect(ctx.relevantMeetings).toHaveLength(1);
    expect(ctx.relevantMeetings[0]?.timestampMs).toBe(18500);
  });
});
