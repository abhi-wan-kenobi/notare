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
  getCurrentWebviewWindow: () => ({ startDragging: mocks.startDragging }),
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

  it("shows the batch-mode hint only while dictating in batch-paste mode", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    // Idle: no hint regardless of mode.
    expect(screen.queryByTestId("dictation-batch-hint")).toBeNull();

    await pushState({ phase: "listening", amplitude: 0.4, mode: "batch-paste" });
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

    await pushState({ phase: "idle", amplitude: 0, mode: "batch-paste" });
    expect(screen.queryByTestId("dictation-batch-hint")).toBeNull();
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
