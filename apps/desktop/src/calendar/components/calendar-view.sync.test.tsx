import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  syncRange: vi.fn(async () => undefined),
  canSync: true,
}));

vi.mock("./context", () => ({
  useSync: () => ({
    canSync: mocks.canSync,
    syncRange: mocks.syncRange,
    scheduleSync: vi.fn(),
    cancelDebouncedSync: vi.fn(),
    status: "idle",
  }),
}));

import { useVisibleRangeSync } from "./calendar-view";

const RANGE = {
  from: new Date("2026-07-01T00:00:00.000Z"),
  to: new Date("2026-08-01T00:00:00.000Z"),
};

function Harness() {
  useVisibleRangeSync(RANGE, "cal-1");
  return null;
}

/**
 * D4 (2026-07-31): the event list is a live query, but the remote pull for
 * the visible range used to run only on mount/month change - the calendar
 * went stale while open. These pin the re-pull triggers.
 */
describe("useVisibleRangeSync", () => {
  let client: QueryClient;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.canSync = true;
    client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
  });

  afterEach(() => {
    cleanup();
    client.clear();
  });

  it("pulls the visible range on mount", async () => {
    render(
      <QueryClientProvider client={client}>
        <Harness />
      </QueryClientProvider>,
    );

    await waitFor(() => expect(mocks.syncRange).toHaveBeenCalledTimes(1));
    expect(mocks.syncRange).toHaveBeenCalledWith(RANGE, expect.anything());
  });

  it("re-pulls when the window regains focus", async () => {
    render(
      <QueryClientProvider client={client}>
        <Harness />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(mocks.syncRange).toHaveBeenCalledTimes(1));

    act(() => {
      window.dispatchEvent(new Event("focus"));
    });

    await waitFor(() => expect(mocks.syncRange).toHaveBeenCalledTimes(2));
  });

  it("does not pull or listen when syncing is unavailable", async () => {
    mocks.canSync = false;

    render(
      <QueryClientProvider client={client}>
        <Harness />
      </QueryClientProvider>,
    );

    await new Promise((resolve) => setTimeout(resolve, 0));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mocks.syncRange).not.toHaveBeenCalled();
  });
});
