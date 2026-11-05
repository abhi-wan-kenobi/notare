import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  takePendingPair: vi.fn(),
  openNew: vi.fn(),
  emit: undefined as
    | ((payload: { to: string; search: unknown }) => void)
    | undefined,
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));

vi.mock("@hypr/plugin-deeplink2", () => ({
  commands: { takePendingPair: mocks.takePendingPair },
  events: {
    deepLinkEvent: {
      listen: (
        cb: (event: { payload: { to: string; search: unknown } }) => void,
      ) => {
        mocks.emit = (payload) => cb({ payload });
        return Promise.resolve(mocks.unlisten);
      },
    },
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (select: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    select({ openNew: mocks.openNew }),
}));

import { usePairLinkHandler } from "./pair-link";
import { usePendingPairLink } from "./pending-pair-link";

const flush = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

describe("usePairLinkHandler", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.emit = undefined;
    usePendingPairLink.getState().clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("cold start: a link parked before the webview listened is queued and opens Devices", async () => {
    mocks.takePendingPair.mockResolvedValue({ link: "notare://pair#cold" });

    renderHook(() => usePairLinkHandler());
    await flush();

    expect(usePendingPairLink.getState().link).toBe("notare://pair#cold");
    expect(mocks.openNew).toHaveBeenCalledTimes(1);
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "integrations", section: "sync" },
    });
  });

  it("does nothing on startup when no link is parked", async () => {
    mocks.takePendingPair.mockResolvedValue(null);

    renderHook(() => usePairLinkHandler());
    await flush();

    expect(usePendingPairLink.getState().link).toBeNull();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  it("warm: a /pair event drains the parked link", async () => {
    mocks.takePendingPair.mockResolvedValueOnce(null);
    renderHook(() => usePairLinkHandler());
    await flush();

    mocks.takePendingPair.mockResolvedValueOnce({
      link: "notare://pair#warm",
    });
    mocks.emit?.({ to: "/pair", search: { link: "notare://pair#warm" } });
    await flush();

    expect(usePendingPairLink.getState().link).toBe("notare://pair#warm");
    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "integrations", section: "sync" },
    });
  });

  it("ignores OAuth-callback events", async () => {
    mocks.takePendingPair.mockResolvedValue(null);
    renderHook(() => usePairLinkHandler());
    await flush();
    mocks.takePendingPair.mockClear();

    mocks.emit?.({ to: "/billing/refresh", search: {} });
    await flush();

    expect(mocks.takePendingPair).not.toHaveBeenCalled();
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  it("stops listening on unmount", async () => {
    mocks.takePendingPair.mockResolvedValue(null);
    const { unmount } = renderHook(() => usePairLinkHandler());
    await flush();

    unmount();
    await flush();

    expect(mocks.unlisten).toHaveBeenCalledTimes(1);
  });
});
