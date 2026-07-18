import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useInvalidateAudioOnRecordingFinish } from "./use-invalidate-audio-on-finish";

const { useListenerMock } = vi.hoisted(() => ({
  useListenerMock: vi.fn(),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

let sessionMode: "inactive" | "active" | "finalizing" | "running_batch" =
  "inactive";

function setupListener() {
  useListenerMock.mockImplementation(
    (
      selector: (state: {
        getSessionMode: () => typeof sessionMode;
      }) => unknown,
    ) => selector({ getSessionMode: () => sessionMode }),
  );
}

function createWrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useInvalidateAudioOnRecordingFinish", () => {
  let queryClient: QueryClient;
  let invalidateSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    invalidateSpy = vi
      .spyOn(queryClient, "invalidateQueries")
      .mockResolvedValue(undefined);
    sessionMode = "inactive";
    setupListener();
  });

  it("invalidates audio existence and url when a live recording finishes", async () => {
    sessionMode = "active";
    const { rerender } = renderHook(
      () => useInvalidateAudioOnRecordingFinish("session-1"),
      { wrapper: createWrapper(queryClient) },
    );

    expect(invalidateSpy).not.toHaveBeenCalled();

    sessionMode = "finalizing";
    rerender();
    expect(invalidateSpy).not.toHaveBeenCalled();

    sessionMode = "inactive";
    rerender();

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["audio", "session-1", "exist"],
      });
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["audio", "session-1", "url"],
      });
    });
  });

  it("invalidates when a batch import finishes", async () => {
    sessionMode = "running_batch";
    const { rerender } = renderHook(
      () => useInvalidateAudioOnRecordingFinish("session-2"),
      { wrapper: createWrapper(queryClient) },
    );

    sessionMode = "inactive";
    rerender();

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({
        queryKey: ["audio", "session-2", "exist"],
      });
    });
  });

  it("does not invalidate on the initial mount even when inactive", () => {
    sessionMode = "inactive";
    renderHook(() => useInvalidateAudioOnRecordingFinish("session-3"), {
      wrapper: createWrapper(queryClient),
    });

    expect(invalidateSpy).not.toHaveBeenCalled();
  });

  it("does not invalidate while still finalizing", () => {
    sessionMode = "active";
    const { rerender } = renderHook(
      () => useInvalidateAudioOnRecordingFinish("session-4"),
      { wrapper: createWrapper(queryClient) },
    );

    sessionMode = "finalizing";
    rerender();

    expect(invalidateSpy).not.toHaveBeenCalled();
  });
});
