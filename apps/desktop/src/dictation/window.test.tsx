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
  emitHideRequested: vi.fn(async () => undefined),
  startDragging: vi.fn(async () => undefined),
  show: vi.fn(),
  setFocus: vi.fn(),
  showMain: vi.fn(),
  setSize: vi.fn(async () => undefined),
  setPosition: vi.fn(async () => undefined),
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
    dictationOrbHideRequested: { emit: mocks.emitHideRequested },
  },
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    startDragging: mocks.startDragging,
    show: mocks.show,
    setFocus: mocks.setFocus,
    setSize: mocks.setSize,
    setPosition: mocks.setPosition,
  }),
}));

vi.mock("@hypr/plugin-windows", () => ({
  commands: { windowShow: mocks.showMain },
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

  it("preserves the document theme while mounting", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});
    expect(document.documentElement.classList.contains("dark")).toBe(false);

    cleanup();
    document.documentElement.classList.add("dark");

    render(<DictationOrbWindow />);
    await act(async () => {});
    expect(document.documentElement.classList.contains("dark")).toBe(true);
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
      screen.getByRole("button", { name: "Finishing dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "idle", amplitude: 0, mode: "type" });
    expect(orb.dataset.dictationPhase).toBe("idle");
    expect(
      screen.getByRole("button", { name: "Start dictation" }),
    ).not.toBeNull();
  });

  it("emits the hide-request event on right-click while idle", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    const defaultAllowed = fireEvent.contextMenu(
      screen.getByRole("button", { name: "Start dictation" }),
    );

    // preventDefault keeps the browser context menu off the orb.
    expect(defaultAllowed).toBe(false);
    expect(mocks.emitHideRequested).toHaveBeenCalledTimes(1);
    expect(mocks.emitClicked).not.toHaveBeenCalled();
  });

  // While a session is live the orb doubles as the mic indicator - a
  // right-click must never leave the mic running with no visual trace.
  // Covers both live phases so this predicate can never drift from the
  // host's own "dictating" set (`listening` | `processing`) unnoticed.
  it.each(["listening", "processing"] as const)(
    "ignores right-click while dictating (%s)",
    async (phase) => {
      render(<DictationOrbWindow />);
      await pushState({ phase, amplitude: 0.4, mode: "type" });

      fireEvent.contextMenu(
        screen.getByRole("button", {
          name:
            phase === "processing" ? "Finishing dictation" : "Stop dictation",
        }),
      );

      expect(mocks.emitHideRequested).not.toHaveBeenCalled();
    },
  );

  it("emits the orb-clicked event on click", async () => {
    render(<DictationOrbWindow />);
    await act(async () => {});

    fireEvent.click(screen.getByRole("button", { name: "Start dictation" }));

    expect(mocks.emitClicked).toHaveBeenCalledTimes(1);
  });

  // A second stop during post-stop processing is a no-op on the Rust side, so
  // the orb must not present itself as a pressed, clickable "Stop" control.
  it("reports progress instead of offering a stop while finishing", async () => {
    render(<DictationOrbWindow />);
    await pushState({ phase: "listening", amplitude: 0.4, mode: "batch" });
    await pushState({ phase: "processing", amplitude: 0, mode: "batch" });

    const button = screen.getByRole("button", { name: "Finishing dictation" });
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button.getAttribute("aria-disabled")).toBe("true");
    expect(button.getAttribute("aria-pressed")).toBe("false");

    fireEvent.click(button);
    expect(mocks.emitClicked).not.toHaveBeenCalled();

    await pushState({ phase: "idle", amplitude: 0, mode: "batch" });
    expect(button.getAttribute("aria-busy")).toBe("false");
    expect(button.getAttribute("aria-disabled")).toBe("false");

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

  it.each(["cobalt", "particles", "classic", "cobalt-halo", "unknown"])(
    "ignores the saved legacy %s shape without resizing any window",
    async (saved) => {
      mocks.config.dictation_orb_variant = saved;
      render(<DictationOrbWindow />);
      await act(async () => {});

      const waveform = screen.getByTestId("dictation-waveform-orb");
      expect(waveform.style.width).toBe("62px");
      expect(waveform.style.height).toBe("30px");
      expect(screen.queryByTestId("dictation-particle-orb")).toBeNull();
      expect(mocks.setSize).not.toHaveBeenCalled();
      expect(mocks.setPosition).not.toHaveBeenCalled();
    },
  );

  it("toggles without requesting native show/focus or moving DOM focus", async () => {
    render(
      <>
        <input aria-label="Dictation target" />
        <DictationOrbWindow />
      </>,
    );
    await act(async () => {});
    const target = screen.getByRole("textbox");
    target.focus();
    const button = screen.getByRole("button", { name: "Start dictation" });
    const down = new MouseEvent("pointerdown", {
      button: 0,
      bubbles: true,
      cancelable: true,
      screenX: 50,
      screenY: 50,
    });
    fireEvent(button, down);
    firePointer(button, "pointerup");
    fireEvent.click(button);

    expect(down.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(target);
    expect(mocks.emitClicked).toHaveBeenCalledTimes(1);
    expect(mocks.show).not.toHaveBeenCalled();
    expect(mocks.setFocus).not.toHaveBeenCalled();
    expect(mocks.showMain).not.toHaveBeenCalled();
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
    expect(mocks.show).not.toHaveBeenCalled();
    expect(mocks.setFocus).not.toHaveBeenCalled();
    expect(mocks.showMain).not.toHaveBeenCalled();

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
