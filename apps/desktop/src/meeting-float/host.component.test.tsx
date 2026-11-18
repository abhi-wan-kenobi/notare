import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  settings: {
    current: {
      floating_bar_opacity: 0.78,
      live_caption_opacity: 0.3,
      live_caption_width: 440,
      live_caption_line_count: 1,
      live_caption_position: "topCenter",
      live_caption_minimized: true,
    },
  },
  floatingBarShow: vi.fn(async () => ({ status: "ok", data: null })),
  floatingBarHide: vi.fn(async () => ({ status: "ok", data: null })),
  floatingBarUpdate: vi.fn(async () => ({ status: "ok", data: null })),
  liveCaptionHide: vi.fn(async () => ({ status: "ok", data: null })),
  windowShow: vi.fn(async () => ({ status: "ok", data: null })),
  listen: vi.fn(async () => vi.fn()),
  stopHandler: null as (() => void) | null,
  openHandler: null as (() => Promise<void>) | null,
  setSettingValue: vi.fn(async () => undefined),
  setSettingValues: vi.fn(),
  subscribeMeetingFloatData: vi.fn(async () => vi.fn(async () => undefined)),
  listenerState: {
    live: {
      status: "active",
      sessionId: "session-1",
      amplitude: { mic: 0, speaker: 0 },
      degraded: null,
      lastError: null,
      liveTranscriptionActive: true,
    },
    liveSegments: [],
    stop: vi.fn(),
  },
  subscribeListener: vi.fn(() => vi.fn()),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: {
    floatingBarShow: mocks.floatingBarShow,
    floatingBarHide: mocks.floatingBarHide,
    floatingBarUpdate: mocks.floatingBarUpdate,
    liveCaptionHide: mocks.liveCaptionHide,
    windowShow: mocks.windowShow,
  },
  events: {
    floatingBarOpenMain: {
      listen: async (handler: () => Promise<void>) => {
        mocks.openHandler = handler;
        return mocks.listen();
      },
    },
    floatingBarReady: { listen: mocks.listen },
    floatingBarSettingsChange: { listen: mocks.listen },
    floatingBarStop: {
      listen: async (handler: () => void) => {
        mocks.stopHandler = handler;
        return mocks.listen();
      },
    },
  },
}));

vi.mock("./hooks", () => ({
  createMeetingFloatLabelContext: vi.fn(() => undefined),
  loadMeetingFloatData: vi.fn(async () => ({ sessions: {}, humanNames: {} })),
  subscribeMeetingFloatData: mocks.subscribeMeetingFloatData,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: vi.fn(async () => ({
    values: mocks.settings.current,
    hasValues: new Set(Object.keys(mocks.settings.current)),
  })),
  setSettingValue: mocks.setSettingValue,
  useSetSettingValues: () => mocks.setSettingValues,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => true,
  useConfigValues: () => mocks.settings.current,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => mocks.listenerState,
    subscribe: mocks.subscribeListener,
  },
}));

import { FloatingMeetingWindowHost } from "./host";

describe("FloatingMeetingWindowHost", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.stopHandler = null;
    mocks.openHandler = null;
    mocks.settings.current = {
      floating_bar_opacity: 0.78,
      live_caption_opacity: 0.3,
      live_caption_width: 440,
      live_caption_line_count: 1,
      live_caption_position: "topCenter",
      live_caption_minimized: true,
    };
  });

  afterEach(() => {
    cleanup();
  });

  it("updates overlay settings without hiding the active floating panel", async () => {
    const view = render(<FloatingMeetingWindowHost />);

    await waitFor(() => {
      expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
      expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ liveCaptionMinimized: true }),
      );
    });

    mocks.settings.current = {
      ...mocks.settings.current,
      live_caption_minimized: false,
    };
    view.rerender(<FloatingMeetingWindowHost />);

    await waitFor(() => {
      expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ liveCaptionMinimized: false }),
      );
    });
    expect(mocks.floatingBarShow).toHaveBeenCalledOnce();
    expect(mocks.floatingBarHide).not.toHaveBeenCalled();
  });

  it("only the explicit Open app event raises the main window", async () => {
    render(<FloatingMeetingWindowHost />);
    await waitFor(() => expect(mocks.stopHandler).not.toBeNull());
    expect(mocks.windowShow).not.toHaveBeenCalled();

    await act(async () => {
      mocks.stopHandler?.();
    });
    expect(mocks.listenerState.stop).toHaveBeenCalledOnce();
    expect(mocks.windowShow).not.toHaveBeenCalled();

    await act(async () => {
      await mocks.openHandler?.();
    });
    expect(mocks.windowShow).toHaveBeenCalledOnce();
    expect(mocks.windowShow).toHaveBeenCalledWith({ type: "main" });
  });

  it("finishes an in-flight show before applying the hide that follows it", async () => {
    let finishShow = () => {};
    mocks.floatingBarShow.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishShow = () => resolve({ status: "ok", data: null });
        }),
    );
    render(<FloatingMeetingWindowHost />);
    await waitFor(() => expect(mocks.floatingBarShow).toHaveBeenCalledOnce());

    // Recording stops while the native show is still pending.
    const stopped = {
      ...mocks.listenerState,
      live: { ...mocks.listenerState.live, status: "inactive" },
    };
    const listeners = (
      mocks.subscribeListener.mock.calls as unknown as Array<
        [(state: unknown, previous: unknown) => void]
      >
    ).map(([listener]) => listener);
    await act(async () => {
      listeners.forEach((listener) => listener(stopped, mocks.listenerState));
    });
    expect(mocks.floatingBarHide).not.toHaveBeenCalled();

    await act(async () => {
      finishShow();
    });
    await waitFor(() => expect(mocks.floatingBarHide).toHaveBeenCalled());

    // The hide is the last word to the native panel: nothing from the stale
    // recording state is sent after it.
    expect(
      Math.max(...mocks.floatingBarHide.mock.invocationCallOrder),
    ).toBeGreaterThan(
      Math.max(
        ...mocks.floatingBarShow.mock.invocationCallOrder,
        ...mocks.floatingBarUpdate.mock.invocationCallOrder,
      ),
    );
  });

  it("sends a collapsed caption area while live transcription is unavailable", async () => {
    mocks.settings.current = {
      ...mocks.settings.current,
      live_caption_minimized: false,
    };
    mocks.listenerState.live.liveTranscriptionActive = false;
    try {
      render(<FloatingMeetingWindowHost />);

      await waitFor(() => {
        expect(mocks.floatingBarUpdate).toHaveBeenLastCalledWith(
          expect.objectContaining({
            liveCaptionMinimized: true,
            liveCaptionToggleVisible: false,
          }),
        );
      });
    } finally {
      mocks.listenerState.live.liveTranscriptionActive = true;
    }
  });
});
