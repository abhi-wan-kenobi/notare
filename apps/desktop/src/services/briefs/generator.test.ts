import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  briefGenerationRateLimiter,
  generateBriefForEvent,
  regenerateBriefForEvent,
} from "./generator";
import type { BriefCandidateEvent } from "./types";

const mocks = vi.hoisted(() => ({
  generateText: vi.fn(),
  savedBriefs: [] as unknown[],
  retrievedContext: {
    attendees: [{ name: "Alice", email: "alice@acme.com", org: "Acme" }],
    relevantMeetings: [
      {
        sessionId: "session-1",
        title: "Sprint Planning",
        date: "2026-10-01",
        outcome: "Decided to ship v1",
        timestampMs: 0,
      },
    ],
    openActionItems: [
      { text: "Follow up with client", sourceSessionId: "session-1" },
    ],
    orgMentions: [],
  },
}));

vi.mock("ai", () => ({
  generateText: (...args: unknown[]) => mocks.generateText(...args),
}));

vi.mock("./prompt", () => ({
  retrieveBriefContext: vi.fn(async () => mocks.retrievedContext),
  assembleBriefPrompt: vi.fn(() => ({
    system: "System prompt for brief",
    prompt: "User prompt for brief",
  })),
}));

vi.mock("./storage", () => ({
  saveBrief: vi.fn(async (brief: unknown) => {
    mocks.savedBriefs.push(brief);
    return {
      id: "brief-123",
      session_id: "",
      event_id: "event-1",
      generated_at: new Date().toISOString(),
      content_md: "Generated brief markdown",
      model: "notes-scope-model",
      dismissed_at: null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }),
  loadBriefForEvent: vi.fn(async () => null),
}));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: vi.fn(async () => [
      {
        id: "event-1",
        title: "Event 1",
        started_at: "2026-11-02T12:00:00.000Z",
        ended_at: "2026-11-02T13:00:00.000Z",
        participants_json: "[]",
      },
    ]),
  },
}));
function withResolvers<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("Generator & Rate Limiting", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.savedBriefs = [];
  });

  it("enforces rate-limiting: only one generation runs at a time", async () => {
    const executionOrder: string[] = [];
    const { promise: p1Gate, resolve: r1Gate } = withResolvers<void>();
    const { promise: p2Gate, resolve: r2Gate } = withResolvers<void>();

    const task1 = async () => {
      const release = await briefGenerationRateLimiter.acquire();
      try {
        executionOrder.push("task1-start");
        expect(briefGenerationRateLimiter.isBusy()).toBe(true);
        await p1Gate;
        executionOrder.push("task1-end");
      } finally {
        release();
      }
    };

    const task2 = async () => {
      const release = await briefGenerationRateLimiter.acquire();
      try {
        executionOrder.push("task2-start");
        await p2Gate;
        executionOrder.push("task2-end");
      } finally {
        release();
      }
    };
    const t1 = task1();
    await Promise.resolve();
    await Promise.resolve();

    const t2 = task2();
    await Promise.resolve();
    await Promise.resolve();

    expect(executionOrder).toEqual(["task1-start"]);

    // Complete task 1
    r1Gate();
    await t1;

    await Promise.resolve();
    await Promise.resolve();
    expect(executionOrder).toEqual(["task1-start", "task1-end", "task2-start"]);

    // Complete task 2
    r2Gate();
    await t2;

    expect(executionOrder).toEqual([
      "task1-start",
      "task1-end",
      "task2-start",
      "task2-end",
    ]);
  });

  it("generates a brief and saves it to storage", async () => {
    mocks.generateText.mockResolvedValueOnce({
      text: "## Summary Brief\nAttendees: Alice\nPrevious meeting: [[src:session-1@0]]",
    });

    const event: BriefCandidateEvent = {
      id: "event-1",
      title: "Sync with Alice",
      started_at: "2026-11-02T14:00:00.000Z",
      ended_at: "2026-11-02T15:00:00.000Z",
      participants_json: JSON.stringify([
        { name: "Alice", email: "alice@acme.com" },
      ]),
    };

    const brief = await generateBriefForEvent(
      event,
      {} as unknown as LanguageModel,
      "test-model",
    );

    expect(brief).toBeDefined();
    expect(mocks.generateText).toHaveBeenCalledWith(
      expect.objectContaining({
        system: "System prompt for brief",
        prompt: "User prompt for brief",
      }),
    );
    expect(mocks.savedBriefs).toHaveLength(1);
    expect(mocks.savedBriefs[0]).toMatchObject({
      event_id: "event-1",
      content_md:
        "## Summary Brief\nAttendees: Alice\nPrevious meeting: [[src:session-1@0]]",
      model: "test-model",
    });
  });

  it("handles empty model response gracefully", async () => {
    mocks.generateText.mockResolvedValueOnce({
      text: "   ",
    });

    const event: BriefCandidateEvent = {
      id: "event-empty",
      title: "Empty Meeting",
      started_at: "2026-11-02T14:00:00.000Z",
      ended_at: "2026-11-02T15:00:00.000Z",
    };

    const brief = await generateBriefForEvent(
      event,
      {} as unknown as LanguageModel,
      "test-model",
    );

    expect(brief).toBeNull();
    expect(mocks.savedBriefs).toHaveLength(0);
  });

  it("regenerates brief by event id", async () => {
    mocks.generateText.mockResolvedValueOnce({
      text: "Regenerated brief content",
    });

    const brief = await regenerateBriefForEvent(
      "event-1",
      {} as unknown as LanguageModel,
      "test-model",
    );

    expect(brief).toBeDefined();
    expect(mocks.generateText).toHaveBeenCalled();
    expect(mocks.savedBriefs).toHaveLength(1);
  });
});
