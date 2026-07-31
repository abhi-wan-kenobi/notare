import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getStoredSettingValues: vi.fn(async () => ({
    values: {},
    hasValues: new Set(),
  })),
  resolveConfigValue: vi.fn(() => "off" as string),
  pruneDictationHistoryByAge: vi.fn(async (_retention: string) => undefined),
  listDictationHistory: vi.fn(async () => ({ entries: [], nextCursor: null })),
}));

vi.mock("~/settings/queries", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/settings/queries")>();
  return {
    ...actual,
    getStoredSettingValues: mocks.getStoredSettingValues,
  };
});

vi.mock("~/shared/config", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/shared/config")>();
  return {
    ...actual,
    resolveConfigValue: mocks.resolveConfigValue,
  };
});

vi.mock("~/dictation/history", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/dictation/history")>();
  return {
    ...actual,
    pruneDictationHistoryByAge: mocks.pruneDictationHistoryByAge,
    listDictationHistory: mocks.listDictationHistory,
  };
});

import { pruneSnippetsHistoryOnLoad, useSnippetsHistoryQuery } from "./queries";

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("pruneSnippetsHistoryOnLoad", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {},
      hasValues: new Set(),
    });
  });

  it("reads dictation_history_retention off the stored settings and forwards it to the age-prune", async () => {
    mocks.resolveConfigValue.mockReturnValue("30d");

    await pruneSnippetsHistoryOnLoad();

    expect(mocks.resolveConfigValue).toHaveBeenCalledWith(
      "dictation_history_retention",
      { values: {}, hasValues: new Set() },
    );
    expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledWith("30d");
  });

  it('still calls through with "off" (a no-op inside pruneDictationHistoryByAge)', async () => {
    mocks.resolveConfigValue.mockReturnValue("off");

    await pruneSnippetsHistoryOnLoad();

    expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledWith("off");
  });
});

describe("useSnippetsHistoryQuery retention prune on load", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {},
      hasValues: new Set(),
    });
    mocks.resolveConfigValue.mockReturnValue("7d");
    mocks.listDictationHistory.mockResolvedValue({
      entries: [],
      nextCursor: null,
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fires the retention prune once when the hook mounts", async () => {
    renderHook(() => useSnippetsHistoryQuery(""), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledTimes(1);
    });
    expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledWith("7d");
  });

  it("does not re-fire the prune when the search query changes", async () => {
    const { rerender } = renderHook(
      ({ query }) => useSnippetsHistoryQuery(query),
      { wrapper: createWrapper(), initialProps: { query: "" } },
    );

    await waitFor(() => {
      expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledTimes(1);
    });

    rerender({ query: "hello" });
    rerender({ query: "hello world" });

    // Give any (incorrect) re-fire a chance to happen before asserting it
    // didn't.
    await waitFor(() => {
      expect(mocks.listDictationHistory).toHaveBeenCalled();
    });
    expect(mocks.pruneDictationHistoryByAge).toHaveBeenCalledTimes(1);
  });
});
