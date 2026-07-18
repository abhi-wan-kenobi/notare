import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { FloatingBarState } from "@hypr/plugin-windows";

const mocks = vi.hoisted(() => ({
  stateHandlers: [] as Array<(event: { payload: unknown }) => void>,
  stateUnlisten: vi.fn(),
  emitStop: vi.fn(async () => undefined),
  emitOpenMain: vi.fn(async () => undefined),
  emitSettingsChange: vi.fn(async () => undefined),
  emitReady: vi.fn(async () => undefined),
  config: {} as Record<string, string | undefined>,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: (keys: readonly string[]) =>
    Object.fromEntries(keys.map((key) => [key, mocks.config[key]])),
}));

vi.mock("@hypr/plugin-windows", () => ({
  events: {
    floatingBarStateEvent: {
      listen: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
        mocks.stateHandlers.push(handler);
        return mocks.stateUnlisten;
      }),
    },
    floatingBarStop: { emit: mocks.emitStop },
    floatingBarOpenMain: { emit: mocks.emitOpenMain },
    floatingBarSettingsChange: { emit: mocks.emitSettingsChange },
    floatingBarReady: { emit: mocks.emitReady },
  },
}));

import { FloatingBarContent, FloatingBarWindow } from "./window";

function makeState(
  overrides: Partial<FloatingBarState> = {},
): FloatingBarState {
  return {
    amplitude: 0.4,
    title: "Weekly sync",
    status: "recording",
    colorScheme: "dark",
    opacity: 0.78,
    liveCaptionOpacity: 0.3,
    liveCaptionWidth: 440,
    liveCaptionLineCount: 1,
    liveCaptionPosition: "topCenter",
    liveCaptionMinimized: true,
    liveCaptionToggleVisible: true,
    transcriptBubbles: [],
    ...overrides,
  };
}

async function pushState(state: FloatingBarState) {
  await act(async () => {
    for (const handler of mocks.stateHandlers) {
      handler({ payload: { state } });
    }
  });
}

describe("FloatingBarWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.stateHandlers.length = 0;
    mocks.config = {};
  });

  afterEach(() => {
    cleanup();
  });

  it("renders nothing until the first state event arrives", async () => {
    const view = render(<FloatingBarWindow />);
    await act(async () => {});

    expect(view.container.innerHTML).toBe("");
    expect(mocks.emitReady).toHaveBeenCalled();

    await pushState(makeState());

    expect(screen.getByText("Weekly sync")).toBeTruthy();
  });

  it("streams live captions when the caption view is expanded", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(
      makeState({
        liveCaptionMinimized: false,
        transcriptBubbles: [
          {
            id: "b1",
            speakerLabel: "You",
            text: "hello there",
            isSelf: true,
            isFinal: true,
            startMs: 0,
            endMs: 900,
            overlapsPrevious: false,
            overlapsNext: false,
          },
          {
            id: "b2",
            speakerLabel: "Speaker 1",
            text: "how are you",
            isSelf: false,
            isFinal: false,
            startMs: 1000,
            endMs: 2000,
            overlapsPrevious: false,
            overlapsNext: false,
          },
        ],
      }),
    );

    const captions = screen.getByTestId("floating-bar-captions");
    expect(captions.textContent).toContain("hello there");
    expect(captions.textContent).toContain("how are you");
    expect(captions.textContent).toContain("Speaker 1");

    await pushState(
      makeState({
        liveCaptionMinimized: false,
        transcriptBubbles: [
          {
            id: "b3",
            speakerLabel: "Speaker 1",
            text: "a newer line",
            isSelf: false,
            isFinal: false,
            startMs: 2100,
            endMs: 2400,
            overlapsPrevious: false,
            overlapsNext: false,
          },
        ],
      }),
    );

    expect(screen.getByTestId("floating-bar-captions").textContent).toContain(
      "a newer line",
    );
  });

  it("hides the caption area and toggle when minimized/not available", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(makeState({ liveCaptionToggleVisible: false }));

    expect(screen.queryByTestId("floating-bar-captions")).toBeNull();
    expect(screen.queryByLabelText("Show captions")).toBeNull();
  });

  it("renders the orb in the listening state while recording", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(makeState());

    const orb = screen.getByTestId("recording-orb");
    expect(orb.getAttribute("data-orb-state")).toBe("listening");
    expect(screen.queryByTestId("recording-orb-error-badge")).toBeNull();
    expect(screen.getByTestId("floating-bar-glass")).toBeTruthy();
  });

  it("switches the orb to the error state", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(makeState({ status: "error" }));

    const orb = screen.getByTestId("recording-orb");
    expect(orb.getAttribute("data-orb-state")).toBe("error");
    expect(screen.getByTestId("recording-orb-error-badge")).toBeTruthy();
  });

  it("mirrors the main window color scheme onto the document root", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(makeState({ colorScheme: "dark" }));
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    await pushState(makeState({ colorScheme: "light" }));
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("renders the solid fallback variant when transparency is unavailable", async () => {
    document.documentElement.style.background = "";
    document.body.style.background = "";

    render(<FloatingBarWindow solid />);
    await act(async () => {});

    await pushState(makeState());

    expect(screen.getByTestId("floating-bar-solid")).toBeTruthy();
    expect(screen.queryByTestId("floating-bar-glass")).toBeNull();
    expect(document.documentElement.style.background).not.toBe("transparent");
  });

  it("emits plugin events from the bar buttons", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});

    await pushState(makeState());

    fireEvent.click(screen.getByLabelText("Stop recording"));
    expect(mocks.emitStop).toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText("Open main window"));
    expect(mocks.emitOpenMain).toHaveBeenCalled();

    fireEvent.click(screen.getByLabelText("Show captions"));
    expect(mocks.emitSettingsChange).toHaveBeenCalledWith(
      expect.objectContaining({ liveCaptionMinimized: false }),
    );

    await pushState(makeState({ liveCaptionMinimized: false }));
    fireEvent.click(screen.getByLabelText("Hide captions"));
    expect(mocks.emitSettingsChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ liveCaptionMinimized: true }),
    );
  });
});

describe("FloatingBarContent orb variant", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.config = {};
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the variant selected in settings", () => {
    mocks.config["dictation_orb_variant"] = "particles";

    render(<FloatingBarContent state={makeState()} />);

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.getAttribute("data-dictation-variant")).toBe("particles");
  });

  it("falls back to the default variant when the setting is unset or invalid", () => {
    const { rerender } = render(<FloatingBarContent state={makeState()} />);
    expect(
      screen
        .getByTestId("dictation-orb")
        .getAttribute("data-dictation-variant"),
    ).toBe("cobalt");

    mocks.config["dictation_orb_variant"] = "not-a-variant";
    rerender(<FloatingBarContent state={makeState()} />);
    expect(
      screen
        .getByTestId("dictation-orb")
        .getAttribute("data-dictation-variant"),
    ).toBe("cobalt");
  });

  it("maps the recording status to the listening phase", () => {
    render(<FloatingBarContent state={makeState({ status: "recording" })} />);

    expect(
      screen.getByTestId("dictation-orb").getAttribute("data-dictation-phase"),
    ).toBe("listening");
  });

  it("maps the error status to the error phase", () => {
    render(<FloatingBarContent state={makeState({ status: "error" })} />);

    expect(
      screen.getByTestId("dictation-orb").getAttribute("data-dictation-phase"),
    ).toBe("error");
  });
});

describe("FloatingBarContent meeting bar theme", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.config = {};
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the Notare bar by default (meeting_bar_theme unset)", () => {
    render(<FloatingBarContent state={makeState()} />);

    expect(screen.getByTestId("floating-bar-glass")).toBeTruthy();
    expect(screen.queryByTestId("classic-bar")).toBeNull();
  });

  it("renders the Notare bar for an explicit 'notare' theme", () => {
    mocks.config["meeting_bar_theme"] = "notare";

    render(<FloatingBarContent state={makeState()} />);

    expect(screen.getByTestId("floating-bar-glass")).toBeTruthy();
    expect(screen.queryByTestId("classic-bar")).toBeNull();
  });

  it("falls back to the Notare bar for an unknown theme value", () => {
    mocks.config["meeting_bar_theme"] = "not-a-theme";

    render(<FloatingBarContent state={makeState()} />);

    expect(screen.getByTestId("floating-bar-glass")).toBeTruthy();
    expect(screen.queryByTestId("classic-bar")).toBeNull();
  });

  it("renders the Classic bar when meeting_bar_theme is 'classic'", () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(<FloatingBarContent state={makeState()} />);

    expect(screen.getByTestId("classic-bar")).toBeTruthy();
    expect(screen.queryByTestId("floating-bar-glass")).toBeNull();
    // Faithful Classic elements: the dancing-bars waveform + stop capsule.
    expect(screen.getByTestId("classic-dancing-bars")).toBeTruthy();
    expect(screen.getByTestId("classic-stop-button")).toBeTruthy();
    // The Notare orb is not part of the Classic bar.
    expect(screen.queryByTestId("dictation-orb")).toBeNull();
  });

  it("shows the ErrorMark instead of DancingBars when the Classic bar errors", () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(<FloatingBarContent state={makeState({ status: "error" })} />);

    expect(screen.getByTestId("classic-error-mark")).toBeTruthy();
    expect(screen.queryByTestId("classic-dancing-bars")).toBeNull();
  });

  it("emits stop from the Classic stop button", () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(<FloatingBarContent state={makeState()} />);

    fireEvent.click(screen.getByLabelText("Stop recording"));
    expect(mocks.emitStop).toHaveBeenCalled();
  });

  it("expands the Classic transcript bubble and reuses the captions surface", async () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(<FloatingBarContent state={makeState()} />);
    await act(async () => {});

    // Compact first: no captions, expand affordance available.
    expect(screen.queryByTestId("floating-bar-captions")).toBeNull();
    expect(
      screen.getByTestId("classic-bar").getAttribute("data-classic-expanded"),
    ).toBeNull();

    fireEvent.click(screen.getByLabelText("Expand live transcript"));
    expect(mocks.emitSettingsChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ liveCaptionMinimized: false }),
    );
  });

  it("collapses the Classic transcript when already expanded", () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(
      <FloatingBarContent state={makeState({ liveCaptionMinimized: false })} />,
    );

    expect(
      screen.getByTestId("classic-bar").getAttribute("data-classic-expanded"),
    ).toBe("true");
    // Expanded reuses the existing FloatingBarCaptions surface.
    expect(screen.getByTestId("floating-bar-captions")).toBeTruthy();

    fireEvent.click(screen.getByLabelText("Collapse live transcript"));
    expect(mocks.emitSettingsChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ liveCaptionMinimized: true }),
    );
  });

  it("hides the Classic expand button when the caption toggle is unavailable", () => {
    mocks.config["meeting_bar_theme"] = "classic";

    render(
      <FloatingBarContent
        state={makeState({ liveCaptionToggleVisible: false })}
      />,
    );

    expect(screen.queryByLabelText("Expand live transcript")).toBeNull();
    expect(screen.queryByLabelText("Collapse live transcript")).toBeNull();
    // Stop capsule still renders (solo width).
    expect(screen.getByTestId("classic-stop-button")).toBeTruthy();
  });
});
