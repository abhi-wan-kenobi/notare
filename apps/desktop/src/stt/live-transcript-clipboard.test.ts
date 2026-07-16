import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LiveTranscriptSegment } from "@hypr/plugin-transcription";

const mocks = vi.hoisted(() => ({
  writeText: vi.fn(async () => undefined),
  getState: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.writeText,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: mocks.getState,
  },
}));

import {
  copyLiveTranscript,
  getFullLiveTranscriptText,
  getLatestLiveTranscriptChunk,
  getLiveTranscriptShortcutHints,
  isLiveTranscriptCopyAvailable,
} from "./live-transcript-clipboard";

function segment(
  id: string,
  text: string,
  startMs: number,
  endMs: number,
  words: string[] = [],
): LiveTranscriptSegment {
  return {
    id,
    text,
    start_ms: startMs,
    end_ms: endMs,
    words: words.map((word) => ({
      text: word,
      is_final: true,
    })),
    key: { channel: "RemoteParty", speaker_index: null },
  } as unknown as LiveTranscriptSegment;
}

describe("getLatestLiveTranscriptChunk", () => {
  it("returns the text of the segment that started last", () => {
    expect(
      getLatestLiveTranscriptChunk({
        liveSegments: [
          segment("a", "hello there", 0, 900),
          segment("b", "how are you", 1000, 2000),
        ],
        liveCaptionText: "hello there how are you",
      }),
    ).toBe("how are you");
  });

  it("prefers word-level text over the segment text", () => {
    expect(
      getLatestLiveTranscriptChunk({
        liveSegments: [
          segment("a", "raw fallback", 0, 900, ["hello", ",", "world", "!"]),
        ],
        liveCaptionText: "",
      }),
    ).toBe("hello, world!");
  });

  it("falls back to the caption text before segments arrive", () => {
    expect(
      getLatestLiveTranscriptChunk({
        liveSegments: [],
        liveCaptionText: "  partial   words ",
      }),
    ).toBe("partial words");
  });

  it("returns an empty string when nothing was transcribed", () => {
    expect(
      getLatestLiveTranscriptChunk({ liveSegments: [], liveCaptionText: "" }),
    ).toBe("");
  });
});

describe("getFullLiveTranscriptText", () => {
  it("prefers the accumulated caption text", () => {
    expect(
      getFullLiveTranscriptText({
        liveSegments: [segment("a", "hello", 0, 500)],
        liveCaptionText: "hello there how are you",
      }),
    ).toBe("hello there how are you");
  });

  it("falls back to segments in chronological order", () => {
    expect(
      getFullLiveTranscriptText({
        liveSegments: [
          segment("b", "second line", 1000, 2000),
          segment("a", "first line", 0, 900),
        ],
        liveCaptionText: "  ",
      }),
    ).toBe("first line\nsecond line");
  });

  it("returns an empty string when nothing was transcribed", () => {
    expect(
      getFullLiveTranscriptText({ liveSegments: [], liveCaptionText: "" }),
    ).toBe("");
  });
});

describe("getLiveTranscriptShortcutHints", () => {
  it("uses mac symbols on mac-like platforms", () => {
    expect(getLiveTranscriptShortcutHints(true)).toEqual({
      latest: "⌘⇧C",
      full: "⌘⇧F",
    });
  });

  it("uses ctrl names elsewhere", () => {
    expect(getLiveTranscriptShortcutHints(false)).toEqual({
      latest: "Ctrl+Shift+C",
      full: "Ctrl+Shift+F",
    });
  });
});

describe("copyLiveTranscript", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reports inactive when no live session is running", async () => {
    mocks.getState.mockReturnValue({
      live: { status: "inactive" },
      liveSegments: [],
      liveCaptionText: "",
    });

    await expect(copyLiveTranscript("latest")).resolves.toBe("inactive");
    expect(mocks.writeText).not.toHaveBeenCalled();
    expect(isLiveTranscriptCopyAvailable()).toBe(false);
  });

  it("reports empty when recording has no text yet", async () => {
    mocks.getState.mockReturnValue({
      live: { status: "active" },
      liveSegments: [],
      liveCaptionText: "",
    });

    await expect(copyLiveTranscript("full")).resolves.toBe("empty");
    expect(mocks.writeText).not.toHaveBeenCalled();
  });

  it("copies the latest chunk to the clipboard", async () => {
    mocks.getState.mockReturnValue({
      live: { status: "active" },
      liveSegments: [
        segment("a", "hello there", 0, 900),
        segment("b", "how are you", 1000, 2000),
      ],
      liveCaptionText: "hello there how are you",
    });

    await expect(copyLiveTranscript("latest")).resolves.toBe("copied");
    expect(mocks.writeText).toHaveBeenCalledWith("how are you");
    expect(isLiveTranscriptCopyAvailable()).toBe(true);
  });

  it("copies the full transcript to the clipboard", async () => {
    mocks.getState.mockReturnValue({
      live: { status: "active" },
      liveSegments: [segment("a", "hello there", 0, 900)],
      liveCaptionText: "hello there how are you",
    });

    await expect(copyLiveTranscript("full")).resolves.toBe("copied");
    expect(mocks.writeText).toHaveBeenCalledWith("hello there how are you");
  });
});
