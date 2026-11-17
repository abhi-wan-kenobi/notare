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
    elapsedSeconds: 0,
    wordCount: 0,
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

  it("renders the shared listening waveform while recording", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});
    await pushState(makeState());

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.getAttribute("data-dictation-phase")).toBe("listening");
    expect(screen.queryByTestId("dictation-waveform-error-badge")).toBeNull();
    expect(screen.getByTestId("floating-bar-glass")).toBeTruthy();
  });

  it("switches the orb to the error state", async () => {
    render(<FloatingBarWindow />);
    await act(async () => {});
    await pushState(makeState({ status: "error" }));

    const orb = screen.getByTestId("dictation-orb");
    expect(orb.getAttribute("data-dictation-phase")).toBe("error");
    expect(screen.getByTestId("dictation-waveform-error-badge")).toBeTruthy();
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

describe("FloatingBarContent compact waveform", () => {
  afterEach(cleanup);

  it.each(["classic", "notare", "unknown"])(
    "ignores the legacy %s theme and particle shape",
    (theme) => {
      mocks.config.meeting_bar_theme = theme;
      mocks.config.dictation_orb_variant = "particles";
      render(<FloatingBarContent state={makeState()} />);

      const orb = screen.getByTestId("dictation-waveform-orb");
      expect(orb.style.width).toBe("62px");
      expect(orb.style.height).toBe("30px");
      expect(screen.queryByTestId("classic-bar")).toBeNull();
      expect(screen.queryByTestId("dictation-particle-orb")).toBeNull();
    },
  );

  it("keeps stop, captions and drag-region clicks separate from Open app", () => {
    vi.clearAllMocks();
    render(<FloatingBarContent state={makeState()} />);
    fireEvent.click(screen.getByLabelText("Stop recording"));
    fireEvent.click(screen.getByLabelText("Show captions"));
    fireEvent.click(screen.getByText("Weekly sync"));

    expect(mocks.emitStop).toHaveBeenCalledTimes(1);
    expect(mocks.emitSettingsChange).toHaveBeenCalledTimes(1);
    expect(mocks.emitOpenMain).not.toHaveBeenCalled();
    fireEvent.click(screen.getByLabelText("Open main window"));
    expect(mocks.emitOpenMain).toHaveBeenCalledTimes(1);
  });
});

describe("FloatingBarContent recording meter", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.config = {};
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the weighted timer with the seconds emphasised", () => {
    render(<FloatingBarContent state={makeState({ elapsedSeconds: 75 })} />);

    const meter = screen.getByTestId("recording-meter");
    // Weighted per §104: dim `mm:`, bold `ss`. Asserted as separate nodes so a
    // regression that flattens the emphasis fails here rather than passing on
    // the concatenated text.
    expect(meter.textContent).toBe("01:15");
    expect(meter.querySelector(".font-semibold")?.textContent).toBe("15");
  });

  it("rolls over into hours", () => {
    render(<FloatingBarContent state={makeState({ elapsedSeconds: 3725 })} />);

    expect(screen.getByTestId("recording-meter").textContent).toBe("1:02:05");
  });

  it("shows the word count once there are words, and hides it at zero", () => {
    const { rerender } = render(
      <FloatingBarContent state={makeState({ wordCount: 0 })} />,
    );
    expect(screen.queryByTestId("recording-word-count")).toBeNull();

    rerender(<FloatingBarContent state={makeState({ wordCount: 128 })} />);
    expect(screen.getByTestId("recording-word-count").textContent).toBe("128");
  });
});
