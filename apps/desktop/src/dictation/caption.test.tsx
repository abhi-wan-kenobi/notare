import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DictationStateEvent } from "@hypr/plugin-dictation";

const mocks = vi.hoisted(() => ({
  transcriptHandlers: [] as Array<(event: { payload: unknown }) => void>,
  stateHandlers: [] as Array<(event: { payload: unknown }) => void>,
  transcriptUnlisten: vi.fn(),
  stateUnlisten: vi.fn(),
  show: vi.fn(async () => undefined),
  hide: vi.fn(async () => undefined),
  config: {
    dictation_caption: true,
  } as Record<string, unknown>,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: (keys: readonly string[]) =>
    Object.fromEntries(keys.map((key) => [key, mocks.config[key]])),
}));

vi.mock("@hypr/plugin-dictation", () => ({
  events: {
    dictationTranscriptEvent: {
      listen: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
        mocks.transcriptHandlers.push(handler);
        return mocks.transcriptUnlisten;
      }),
    },
    dictationStateEvent: {
      listen: vi.fn(async (handler: (event: { payload: unknown }) => void) => {
        mocks.stateHandlers.push(handler);
        return mocks.stateUnlisten;
      }),
    },
  },
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    show: mocks.show,
    hide: mocks.hide,
  }),
}));

import { DictationCaptionWindow } from "./caption";

async function pushTranscript(text: string) {
  await act(async () => {
    for (const handler of mocks.transcriptHandlers) {
      handler({ payload: { text } });
    }
  });
}

async function pushState(state: DictationStateEvent) {
  await act(async () => {
    for (const handler of mocks.stateHandlers) {
      handler({ payload: state });
    }
  });
}

async function advance(ms: number) {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
}

describe("DictationCaptionWindow", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    mocks.transcriptHandlers.length = 0;
    mocks.stateHandlers.length = 0;
    mocks.config.dictation_caption = true;
    document.documentElement.classList.remove("dark");
    document.documentElement.style.background = "";
    document.body.style.background = "";
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("starts invisible and never intercepts pointer events", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    const root = screen.getByTestId("dictation-caption-glass");
    expect(root.className).toContain("pointer-events-none");
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-0");
  });

  it("makes the page transparent for the glass variant only", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});
    expect(document.documentElement.style.background).toBe("transparent");

    cleanup();
    document.documentElement.style.background = "";
    document.body.style.background = "";

    render(<DictationCaptionWindow solid />);
    await act(async () => {});
    expect(document.documentElement.style.background).toBe("");
    expect(screen.getByTestId("dictation-caption-solid")).not.toBeNull();
  });

  it("shows streamed words and the OS window", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("hello world");

    expect(screen.getByTestId("dictation-caption-text").textContent).toBe(
      "hello world",
    );
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-100");
    expect(mocks.show).toHaveBeenCalled();
  });

  it("keeps only the last 10 words", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("one two three four five six seven");
    await pushTranscript("eight nine ten eleven twelve");

    expect(screen.getByTestId("dictation-caption-text").textContent).toBe(
      "three four five six seven eight nine ten eleven twelve",
    );
  });

  it("fades out after ~2s without new words and hides the OS window", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("hello");
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-100");

    await advance(1900);
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-100");

    await advance(200);
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-0");

    // The OS window hides once the opacity transition has played out.
    await advance(300);
    expect(mocks.hide).toHaveBeenCalled();
  });

  it("keeps the caption alive while words keep arriving", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("hello");
    await advance(1500);
    await pushTranscript("again");
    await advance(1500);

    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-100");
  });

  it("fades shortly after the session ends", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("hello");
    await pushState({ phase: "idle", amplitude: 0, mode: "type" });

    await advance(1300);
    expect(
      screen.getByTestId("dictation-caption-bubble").className,
    ).toContain("opacity-0");
  });

  it("clears the previous tail when a new session starts", async () => {
    render(<DictationCaptionWindow />);
    await act(async () => {});

    await pushTranscript("old tail");
    await pushState({ phase: "idle", amplitude: 0, mode: "type" });
    await pushState({ phase: "listening", amplitude: 0.2, mode: "type" });

    expect(screen.getByTestId("dictation-caption-text").textContent).toBe("");

    await pushTranscript("fresh words");
    expect(screen.getByTestId("dictation-caption-text").textContent).toBe(
      "fresh words",
    );
  });

  it("does nothing when the setting is off", async () => {
    mocks.config.dictation_caption = false;
    render(<DictationCaptionWindow />);
    await act(async () => {});

    expect(mocks.transcriptHandlers).toHaveLength(0);
    expect(mocks.stateHandlers).toHaveLength(0);
  });

  it("unsubscribes on unmount", async () => {
    const view = render(<DictationCaptionWindow />);
    await act(async () => {});

    view.unmount();

    expect(mocks.transcriptUnlisten).toHaveBeenCalledTimes(1);
    expect(mocks.stateUnlisten).toHaveBeenCalledTimes(1);
  });
});
