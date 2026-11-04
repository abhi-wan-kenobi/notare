import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { checkAndGenerateBriefs, selectCandidateEvents } from "./scheduler";
import type { BriefCandidateEvent } from "./types";

import type * as HiddenProviders from "~/calendar/hidden-providers";

const mocks = vi.hoisted(() => ({
  briefedIds: new Set<string>(),
  calendarEvents: [] as BriefCandidateEvent[],
  generatedBriefs: [] as string[],
}));

vi.mock("./storage", () => ({
  loadBriefedEventIds: async () => mocks.briefedIds,
}));

vi.mock("./generator", () => ({
  generateBriefForEvent: vi.fn(async (event: BriefCandidateEvent) => {
    mocks.generatedBriefs.push(event.id);
    return {
      id: `brief-${event.id}`,
      session_id: "",
      event_id: event.id,
      generated_at: new Date().toISOString(),
      content_md: "Sample brief",
      model: "test-model",
      dismissed_at: null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }),
}));

vi.mock("~/calendar/hidden-providers", async (importOriginal) => ({
  ...(await importOriginal<typeof HiddenProviders>()),
  getHiddenCalendarProviders: async () => [],
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: vi.fn(async () => mocks.calendarEvents),
  },
}));

describe("Candidate Selection", () => {
  const baseTime = new Date("2026-11-02T10:00:00.000Z").getTime();
  const leadTimeHours = 12;

  const validAttendeeJson = JSON.stringify([
    { name: "John Doe", email: "john@example.com", is_current_user: true },
    { name: "Alice Smith", email: "alice@acme.com", is_current_user: false },
  ]);

  it("selects events starting within the lead time window", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-1",
        title: "Sprint Planning",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-past",
        title: "Yesterday's Meeting",
        started_at: new Date(baseTime - 1 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime - 0.5 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-too-far",
        title: "Next Week Meeting",
        started_at: new Date(baseTime + 24 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 25 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );

    expect(candidates).toHaveLength(1);
    expect(candidates[0]?.id).toBe("event-1");
  });

  it("excludes all-day events", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-all-day",
        title: "Company Holiday",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 10 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
        is_all_day: true,
      },
      {
        id: "event-all-day-num",
        title: "Offsite",
        started_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 9 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
        is_all_day: 1,
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );
    expect(candidates).toHaveLength(0);
  });

  it("requires at least 1 attendee (excluding current user)", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-solo",
        title: "Focus Time",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: JSON.stringify([
          {
            name: "John Doe",
            email: "john@example.com",
            is_current_user: true,
          },
        ]),
      },
      {
        id: "event-empty",
        title: "Blocker",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: "[]",
      },
      {
        id: "event-with-attendee",
        title: "1:1 with Bob",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: JSON.stringify([
          { name: "Bob Jones", email: "bob@acme.com", is_current_user: false },
        ]),
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );
    expect(candidates).toHaveLength(1);
    expect(candidates[0]?.id).toBe("event-with-attendee");
  });

  it("excludes events that are already briefed", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-already-briefed",
        title: "Design Review",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-not-briefed",
        title: "Code Review",
        started_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 4 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
    ];

    const alreadyBriefed = new Set(["event-already-briefed"]);
    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      alreadyBriefed,
    );

    expect(candidates).toHaveLength(1);
    expect(candidates[0]?.id).toBe("event-not-briefed");
  });

  it("dedupes candidates by event id", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-duplicate",
        title: "Sync 1",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-duplicate",
        title: "Sync 1 Copy",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-other",
        title: "Sync 2",
        started_at: new Date(baseTime + 4 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 5 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );
    expect(candidates).toHaveLength(2);
    expect(candidates.map((c) => c.id)).toEqual([
      "event-duplicate",
      "event-other",
    ]);
  });

  it("skips events with malformed started_at or ended_at timestamps without throwing", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-bad-start",
        title: "Bad Start Date",
        started_at: "not-a-date",
        ended_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-empty-start",
        title: "Empty Start Date",
        started_at: "",
        ended_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
      {
        id: "event-bad-end",
        title: "Bad End Date",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: "garbage-end-time",
        participants_json: validAttendeeJson,
      },
      {
        id: "event-valid",
        title: "Valid Event",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: validAttendeeJson,
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );
    expect(candidates).toHaveLength(1);
    expect(candidates[0]?.id).toBe("event-valid");
  });

  it("skips events with malformed participants_json without throwing", () => {
    const events: BriefCandidateEvent[] = [
      {
        id: "event-bad-json",
        title: "Bad JSON",
        started_at: new Date(baseTime + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(baseTime + 3 * 3600 * 1000).toISOString(),
        participants_json: "{not: valid; json}",
      },
    ];

    const candidates = selectCandidateEvents(
      events,
      baseTime,
      leadTimeHours,
      new Set(),
    );
    expect(candidates).toHaveLength(0);
  });
});

describe("checkAndGenerateBriefs", () => {
  beforeEach(() => {
    mocks.briefedIds.clear();
    mocks.generatedBriefs = [];
  });

  it("skips silently when enabled is false", async () => {
    const count = await checkAndGenerateBriefs({
      enabled: false,
      model: {} as unknown as LanguageModel,
    });
    expect(count).toBe(0);
    expect(mocks.generatedBriefs).toHaveLength(0);
  });

  it("skips silently when model is null", async () => {
    const count = await checkAndGenerateBriefs({
      enabled: true,
      model: null,
    });
    expect(count).toBe(0);
    expect(mocks.generatedBriefs).toHaveLength(0);
  });

  it("generates briefs for eligible candidates", async () => {
    const now = new Date("2026-11-02T10:00:00.000Z").getTime();
    mocks.calendarEvents = [
      {
        id: "event-candidate-1",
        title: "Product Sync",
        started_at: new Date(now + 2 * 3600 * 1000).toISOString(),
        ended_at: new Date(now + 3 * 3600 * 1000).toISOString(),
        participants_json: JSON.stringify([
          { name: "Alice", email: "alice@test.com", is_current_user: false },
        ]),
        is_all_day: 0,
      },
    ];

    const count = await checkAndGenerateBriefs({
      enabled: true,
      model: {} as unknown as LanguageModel,
      modelName: "test-model",
      now,
    });

    expect(count).toBe(1);
    expect(mocks.generatedBriefs).toContain("event-candidate-1");
  });
});
