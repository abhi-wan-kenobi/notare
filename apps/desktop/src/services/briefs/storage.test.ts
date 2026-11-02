import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(),
  liveQueryExecute: vi.fn(),
}));

vi.mock("~/db", () => ({
  executeTransaction: (...args: unknown[]) => mocks.executeTransaction(...args),
  liveQueryClient: {
    execute: (...args: unknown[]) => mocks.liveQueryExecute(...args),
  },
  useLiveQuery: vi.fn(),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, task: () => Promise<unknown>) => task(),
}));

import { dismissBrief, saveBrief } from "./storage";

describe("Brief Storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("rejects saving a brief with an empty event_id", async () => {
    await expect(
      saveBrief({
        event_id: "",
        content_md: "Some brief",
        model: "ollama/llama3",
      }),
    ).rejects.toThrow("event_id is required");

    await expect(
      saveBrief({
        event_id: "   ",
        content_md: "Some brief",
        model: "ollama/llama3",
      }),
    ).rejects.toThrow("event_id is required");
  });

  it("saves a valid brief successfully", async () => {
    mocks.liveQueryExecute.mockResolvedValueOnce([]); // no existing brief
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    const brief = await saveBrief({
      event_id: "event-valid-1",
      content_md: "## Summary",
      model: "test-model",
    });

    expect(brief.event_id).toBe("event-valid-1");
    expect(brief.content_md).toBe("## Summary");
    expect(mocks.executeTransaction).toHaveBeenCalledOnce();
  });

  it("dismisses an existing brief by updating dismissed_at", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await dismissBrief("brief-123");

    expect(mocks.executeTransaction).toHaveBeenCalledWith([
      expect.objectContaining({
        sql: expect.stringContaining("UPDATE session_briefs SET dismissed_at"),
        params: expect.arrayContaining(["brief-123"]),
      }),
    ]);
  });
});
