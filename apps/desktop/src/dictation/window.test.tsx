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

import { DictationOrbWindow } from "./window";

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
    await pushState({ phase: "listening", amplitude: 0.8 });

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.dataset.dictationPhase).toBe("listening");
    expect(
      screen.getByRole("button", { name: "Stop dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "processing", amplitude: 0 });
    expect(orb.dataset.dictationPhase).toBe("processing");
    expect(
      screen.getByRole("button", { name: "Stop dictation" }),
    ).not.toBeNull();

    await pushState({ phase: "idle", amplitude: 0 });
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

  it("unsubscribes from state events on unmount", async () => {
    const view = render(<DictationOrbWindow />);
    await act(async () => {});

    view.unmount();

    expect(mocks.stateUnlisten).toHaveBeenCalledTimes(1);
  });
});
