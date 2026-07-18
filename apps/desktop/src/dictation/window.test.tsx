import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DictationStateEvent } from "@hypr/plugin-dictation";

const mocks = vi.hoisted(() => ({
  stateHandlers: [] as Array<(event: { payload: unknown }) => void>,
  stateUnlisten: vi.fn(),
  emitClicked: vi.fn(async () => undefined),
  startDragging: vi.fn(async () => undefined),
  scaleFactor: vi.fn(async () => 1),
  // Rust creates the orb window at the cobalt size (56 logical px).
  innerSize: vi.fn(async () => ({
    toLogical: () => ({ width: 56, height: 56 }),
  })),
  outerPosition: vi.fn(async () => ({
    toLogical: () => ({ x: 200, y: 300 }),
  })),
  setSize: vi.fn(async () => undefined),
  setPosition: vi.fn(async () => undefined),
  // Orb window visibility (Settings→Dictation "half-page" repro, #Windows):
  // a variant pick must never resize the orb window while it's hidden -
  // default true so the existing size-sync tests below (which don't care
  // about visibility) keep exercising the resize path unchanged.
  orbWindowVisible: true,
  isVisible: vi.fn(async () => mocks.orbWindowVisible),
  monitor: null as null | {
    position: { x: number; y: number };
    size: { width: number; height: number };
    scaleFactor: number;
  },
  config: {
    dictation_orb_variant: "cobalt",
    dictation_paste_at_cursor: true,
  } as Record<string, unknown>,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: (keys: readonly string[]) =>
    Object.fromEntries(keys.map((key) => [key, mocks.config[key]])),
}));

vi.mock("@hypr/plugin-dictation", () => ({
  events: {
    dictationStateEvent: {
      listen: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
        mocks.stateHandlers.push(handler);
        return mocks.stateUnlisten;
      }),
    },
    dictationOrbClicked: { emit: mocks.emitClicked },
  },
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    startDragging: mocks.startDragging,
    scaleFactor: mocks.scaleFactor,
    innerSize: mocks.innerSize,
    outerPosition: mocks.outerPosition,
    setSize: mocks.setSize,
    setPosition: mocks.setPosition,
    isVisible: mocks.isVisible,
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  currentMonitor: vi.fn(async () => mocks.monitor),
}));

import { DictationOrbWindow } from "./window";

/**
 * jsdom has no PointerEvent constructor, so fireEvent.pointerDown/Move drops
 * coordinate props. Dispatch MouseEvents with pointer event types instead -
 * React routes them by type and the component only reads screenX/screenY.
 */
function firePointer(
  element: Element,
  type: "pointerdown" | "pointermove" | "pointerup",
  init: MouseEventInit = {},
) {
  fireEvent(
    element,
    new MouseEvent(type, { bubbles: true, cancelable: true, ...init }),
  );
}

async function pushState(state: DictationStateEvent) {
  await act(async () => {
    for (const handler of mocks.stateHandlers) {
      handler({ payload: state });
    }
  });
}

describe("DictationOrbWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.stateHandlers.length = 0;
    mocks.monitor = null;
    mocks.orbWindowVisible = true;
    document.documentElement.classList.remove("dark");
    document.documentElement.style.background = "";
    document.body.style.background = "";
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the idle orb before any state event arrives", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.dataset.dictationPhase).toBe("idle");
    expect(
      screen.getByRole("button", { name: "Start dictation" }),
    ).not.toBeNull();
  });

  it("makes the page transparent for the glass variant only", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    expect(document.documentElement.style.background).toBe("transparent");
    expect(document.body.style.background).toBe("transparent");

    cleanup();
    document.documentElement.style.background = "";
    document.body.style.background = "";

    render(<DictationOrbWindow solid />);
    await act(async () => {});

    expect(document.documentElement.style.background).toBe("");
    expect(screen.getByTestId("dictation-window-solid")).not.toBeNull();
  });

  it("tracks phase and amplitude from state events", async () => {
    render(<DictationOrbWindow />);
    await pushState({ phase: "listening", amplitude: 0.8, mode: "type" });

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.dataset.dictationPhase).toBe("listening");
    expect(
      screen.getByRole("button", { name: "Stop dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "processing", amplitude: 0, mode: "type" });
    expect(orb.dataset.dictationPhase).toBe("processing");
    expect(
      screen.getByRole("button", { name: "Stop dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "idle", amplitude: 0, mode: "type" });
    expect(orb.dataset.dictationPhase).toBe("idle");
    expect(
      screen.getByRole("button", { name: "Start dictation" }),
    ).not.toBeNull();
  });

  it("emits the orb-clicked event on click", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    fireEvent.click(screen.getByRole("button", { name: "Start dictation" }));

    expect(mocks.emitClicked).toHaveBeenCalledTimes(1);
  });

  it("shows the batch-mode hint only while dictating in batch mode", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    // Idle: no hint regardless of mode.
    expect(screen.queryByTestId("dictation-batch-hint")).toBeNull();

    await pushState({ phase: "listening", amplitude: 0.4, mode: "batch" });
    expect(screen.getByTestId("dictation-batch-hint")).not.toBeNull();
    expect(
      screen.getByRole("button", {
        name: "Stop dictation and paste the transcript",
      }),
    ).not.toBeNull();

    await pushState({ phase: "listening", amplitude: 0.4, mode: "type" });
    expect(screen.queryByTestId("dictation-batch-hint")).toBeNull();
    expect(
      screen.getByRole("button", { name: "Stop dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "idle", amplitude: 0, mode: "batch" });
    expect(screen.queryByTestId("dictation-batch-hint")).toBeNull();
  });

  it("labels a copy-only batch stop accordingly", async () => {
    mocks.config.dictation_paste_at_cursor = false;
    try {
      render(<DictationOrbWindow />);
      await pushState({ phase: "listening", amplitude: 0.4, mode: "batch" });

      expect(
        screen.getByRole("button", {
          name: "Stop dictation and copy the transcript",
        }),
      ).not.toBeNull();
    } finally {
      mocks.config.dictation_paste_at_cursor = true;
    }
  });

  it("renders the particle orb variant when configured", async () => {
    mocks.config.dictation_orb_variant = "particles";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});

      expect(
        screen.getByTestId("dictation-orb").dataset.dictationVariant,
      ).toBe("particles");
      expect(screen.getByTestId("dictation-particle-orb")).not.toBeNull();
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("renders the Pulse waveform variant when configured", async () => {
    mocks.config.dictation_orb_variant = "waveform";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});

      expect(
        screen.getByTestId("dictation-orb").dataset.dictationVariant,
      ).toBe("waveform");
      expect(screen.getByTestId("dictation-waveform-orb")).not.toBeNull();
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("keeps the created window size for the cobalt variant", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    // Already 56x56 - no resize round-trip.
    expect(mocks.setSize).not.toHaveBeenCalled();
    expect(mocks.setPosition).not.toHaveBeenCalled();
  });

  it("grows the window around its center for the particles variant", async () => {
    mocks.config.dictation_orb_variant = "particles";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});

      // 56 -> 84 logical px (1.5x), re-centered on the old spot.
      expect(mocks.setSize).toHaveBeenCalledTimes(1);
      expect(mocks.setSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 84, height: 84 }),
      );
      expect(mocks.setPosition).toHaveBeenCalledWith(
        expect.objectContaining({ x: 186, y: 286 }),
      );
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("clamps the grown window to the monitor near a screen edge", async () => {
    mocks.config.dictation_orb_variant = "particles";
    mocks.monitor = {
      position: { x: 0, y: 0 },
      size: { width: 1920, height: 1080 },
      scaleFactor: 1,
    };
    // Orb parked in the bottom-right corner: growing 56 -> 84 around the
    // center would push it past both edges without the clamp.
    mocks.outerPosition.mockImplementation(async () => ({
      toLogical: () => ({ x: 1880, y: 1040 }),
    }));
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});

      expect(mocks.setSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 84, height: 84 }),
      );
      expect(mocks.setPosition).toHaveBeenCalledWith(
        expect.objectContaining({ x: 1836, y: 996 }),
      );
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("does not resize the orb window for a variant picked while it is hidden", async () => {
    // Settings→Dictation "half-page" repro (Windows/WebView2): a native
    // setSize/setPosition on the orb window - even one that is hidden -
    // shares the single-threaded window event loop with every other
    // webview, including Settings. A variant pick that lands while the orb
    // is off/hidden must never reach the native call.
    mocks.orbWindowVisible = false;
    mocks.config.dictation_orb_variant = "particles";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});

      expect(mocks.setSize).not.toHaveBeenCalled();
      expect(mocks.setPosition).not.toHaveBeenCalled();
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("applies a deferred resize once a dictation-session event proves the window is shown", async () => {
    mocks.orbWindowVisible = false;
    mocks.config.dictation_orb_variant = "particles";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});
      expect(mocks.setSize).not.toHaveBeenCalled();

      // The orb window is only ever shown for an enabled/active dictation
      // session, so a state event is the signal that it's now on screen.
      mocks.orbWindowVisible = true;
      await pushState({ phase: "listening", amplitude: 0.4, mode: "type" });

      expect(mocks.setSize).toHaveBeenCalledTimes(1);
      expect(mocks.setSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 84, height: 84 }),
      );
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("does not re-resize on a session event once nothing is pending", async () => {
    mocks.config.dictation_orb_variant = "particles";
    try {
      render(<DictationOrbWindow />);
      await act(async () => {});
      expect(mocks.setSize).toHaveBeenCalledTimes(1);

      // The window was already visible, so the initial pick applied
      // immediately - a later session event must not resize it again.
      await pushState({ phase: "listening", amplitude: 0.4, mode: "type" });
      expect(mocks.setSize).toHaveBeenCalledTimes(1);
    } finally {
      mocks.config.dictation_orb_variant = "cobalt";
    }
  });

  it("starts a window drag once the pointer travels past the threshold", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    const button = screen.getByRole("button", { name: "Start dictation" });

    firePointer(button, "pointerdown", {
      button: 0,
      screenX: 100,
      screenY: 100,
    });
    // Below the 4px threshold: no drag yet.
    firePointer(button, "pointermove", { screenX: 102, screenY: 101 });
    expect(mocks.startDragging).not.toHaveBeenCalled();

    firePointer(button, "pointermove", { screenX: 106, screenY: 103 });
    expect(mocks.startDragging).toHaveBeenCalledTimes(1);

    // Further movement must not start a second drag.
    firePointer(button, "pointermove", { screenX: 140, screenY: 120 });
    expect(mocks.startDragging).toHaveBeenCalledTimes(1);

    // A click that trails the drag gesture must NOT toggle dictation.
    fireEvent.click(button);
    expect(mocks.emitClicked).not.toHaveBeenCalled();

    // The next plain click toggles again.
    firePointer(button, "pointerdown", {
      button: 0,
      screenX: 140,
      screenY: 120,
    });
    firePointer(button, "pointerup");
    fireEvent.click(button);
    expect(mocks.emitClicked).toHaveBeenCalledTimes(1);
  });

  it("treats sub-threshold pointer jitter as a click", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    const button = screen.getByRole("button", { name: "Start dictation" });

    firePointer(button, "pointerdown", { button: 0, screenX: 50, screenY: 50 });
    firePointer(button, "pointermove", { screenX: 51, screenY: 52 });
    firePointer(button, "pointerup");
    fireEvent.click(button);

    expect(mocks.startDragging).not.toHaveBeenCalled();
    expect(mocks.emitClicked).toHaveBeenCalledTimes(1);
  });

  it("unsubscribes from state events on unmount", async () => {
    const view = render(<DictationOrbWindow />);
    await act(async () => {});

    view.unmount();

    expect(mocks.stateUnlisten).toHaveBeenCalledTimes(1);
  });
});
