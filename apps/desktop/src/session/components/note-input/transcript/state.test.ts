import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useTranscriptScreen } from "./state";

const { useListenerMock, useAudioPlayerMock, useSessionTranscriptsMock } =
  vi.hoisted(() => ({
    useListenerMock: vi.fn(),
    useAudioPlayerMock: vi.fn(),
    useSessionTranscriptsMock: vi.fn(),
  }));

vi.mock("~/stt/queries", () => ({
  useSessionTranscripts: useSessionTranscriptsMock,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("~/audio-player", () => ({
  useAudioPlayer: useAudioPlayerMock,
}));

type SessionMode = "inactive" | "active" | "finalizing" | "running_batch";

describe("useTranscriptScreen", () => {
  const sessionId = "session-1";
  const liveSegment = {
    id: "segment-1",
    key: { channel: "DirectMic" },
    start_ms: 0,
    end_ms: 1000,
    text: "hello",
    words: [],
  };

  let listenerState: {
    getSessionMode: (id: string) => SessionMode;
    batch: Record<
      string,
      { error?: string; terminalReason?: "failed" | "stopped" }
    >;
    live: {
      sessionId: string | null;
      degraded: null;
      requestedLiveTranscription: boolean;
      liveTranscriptionActive: boolean;
    };
    liveSegments: unknown[];
  };
  let transcripts: Array<{ id: string; words: unknown[] }>;

  beforeEach(() => {
    transcripts = [];
    listenerState = {
      getSessionMode: () => "inactive",
      batch: {},
      live: {
        sessionId: null,
        degraded: null,
        requestedLiveTranscription: true,
        liveTranscriptionActive: true,
      },
      liveSegments: [],
    };

    useSessionTranscriptsMock.mockImplementation(() => transcripts);
    useListenerMock.mockImplementation((selector) => selector(listenerState));
    useAudioPlayerMock.mockReturnValue({ audioExists: true });
  });

  it("shows the error screen when a batch fails and there is no transcript", () => {
    listenerState.batch = {
      [sessionId]: { error: "decode failed", terminalReason: "failed" },
    };

    const { result } = renderHook(() => useTranscriptScreen({ sessionId }));

    expect(result.current).toEqual({
      kind: "empty",
      hasAudio: true,
      error: "decode failed",
    });
  });

  it("keeps the existing transcript and carries the error when a batch fails", () => {
    transcripts = [{ id: "transcript-1", words: [{ id: "word-1" }] }];
    listenerState.batch = {
      [sessionId]: { error: "decode failed", terminalReason: "failed" },
    };

    const { result } = renderHook(() => useTranscriptScreen({ sessionId }));

    expect(result.current).toMatchObject({
      kind: "ready",
      transcriptIds: ["transcript-1"],
      error: "decode failed",
    });
  });

  it("does not report a user-stopped transcription as a failure", () => {
    listenerState.batch = {
      [sessionId]: {
        error: "Transcription stopped.",
        terminalReason: "stopped",
      },
    };

    const { result } = renderHook(() => useTranscriptScreen({ sessionId }));

    expect(result.current).toEqual({
      kind: "empty",
      hasAudio: true,
      error: null,
    });
  });

  it("ignores live segments that belong to another session", () => {
    listenerState.live.sessionId = "session-2";
    listenerState.liveSegments = [liveSegment];

    const { result } = renderHook(() => useTranscriptScreen({ sessionId }));

    expect(result.current.kind).toBe("empty");
  });

  it("shows live segments for the session being recorded", () => {
    listenerState.getSessionMode = () => "active";
    listenerState.live.sessionId = sessionId;
    listenerState.liveSegments = [liveSegment];

    const { result } = renderHook(() => useTranscriptScreen({ sessionId }));

    expect(result.current).toMatchObject({
      kind: "ready",
      liveSegments: [liveSegment],
      currentActive: true,
    });
  });
});
