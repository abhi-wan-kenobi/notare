import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { isAudioUploadFile, useUploadFile } from "./useUploadFile";

const {
  audioImportDataMock,
  audioImportMock,
  audioImportListenMock,
  audioSourceMetadataMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  parseSubtitleMock,
  createTranscriptMock,
  enhanceMock,
  handleBatchFailedMock,
  handleBatchStartedMock,
  updateBatchProgressMock,
  clearBatchSessionMock,
  runBatchMock,
  useSessionMock,
  updateSessionMock,
  useTabsMock,
  updateSessionTabStateMock,
} = vi.hoisted(() => ({
  audioImportDataMock: vi.fn(),
  audioImportMock: vi.fn(),
  audioImportListenMock: vi.fn(),
  audioSourceMetadataMock: vi.fn(),
  parseSubtitleMock: vi.fn(),
  createTranscriptMock: vi.fn(),
  enhanceMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  handleBatchStartedMock: vi.fn(),
  updateBatchProgressMock: vi.fn(),
  clearBatchSessionMock: vi.fn(),
  runBatchMock: vi.fn(),
  useSessionMock: vi.fn(),
  updateSessionMock: vi.fn(),
  useTabsMock: vi.fn(),
  updateSessionTabStateMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: vi.fn(),
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioImport: audioImportMock,
    audioImportData: audioImportDataMock,
    audioSourceMetadata: audioSourceMetadataMock,
  },
  events: {
    audioImportEvent: {
      listen: audioImportListenMock,
    },
  },
}));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: { parseSubtitle: parseSubtitleMock },
}));

vi.mock("./contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      handleBatchStarted: handleBatchStartedMock,
      handleBatchFailed: handleBatchFailedMock,
      updateBatchProgress: updateBatchProgressMock,
      clearBatchSession: clearBatchSessionMock,
    }),
}));

vi.mock("./useRunBatch", () => ({
  isStoppedTranscriptionError: vi.fn(() => false),
  useRunBatch: vi.fn(() => runBatchMock),
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: vi.fn(() => ({
    enhance: enhanceMock,
    queueAutoEnhanceIfSummaryEmpty: queueAutoEnhanceIfSummaryEmptyMock,
  })),
}));

vi.mock("~/session/queries", () => ({
  useSession: useSessionMock,
  useUpdateSession: () => updateSessionMock,
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: useTabsMock,
}));

vi.mock("~/stt/queries", () => ({
  createTranscript: createTranscriptMock,
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe("useUploadFile", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    audioImportDataMock.mockResolvedValue({
      status: "ok",
      data: "/vault/sessions/session-1/audio.wav",
    });
    audioImportMock.mockResolvedValue({
      status: "ok",
      data: "/vault/sessions/session-1/audio.mp3",
    });
    audioSourceMetadataMock.mockResolvedValue({
      status: "error",
      error: "not available",
    });
    queueAutoEnhanceIfSummaryEmptyMock.mockResolvedValue({ type: "queued" });
    audioImportListenMock.mockResolvedValue(vi.fn());
    runBatchMock.mockResolvedValue(undefined);
    createTranscriptMock.mockResolvedValue(undefined);
    enhanceMock.mockResolvedValue({ type: "started", noteId: "note-1" });
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "",
      event_json: "",
    });
    updateSessionMock.mockResolvedValue(undefined);
    useTabsMock.mockImplementation((selector) =>
      selector({
        tabs: [],
        updateSessionTabState: updateSessionTabStateMock,
      }),
    );
  });

  test("imports pathless dropped audio using file bytes", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });
    const file = new File([new Uint8Array([1, 2, 3])], "drop.wav", {
      type: "audio/wav",
      lastModified: 1_700_000_000_000,
    });
    Object.defineProperty(file, "arrayBuffer", {
      value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
    });

    act(() => {
      result.current.processAudioFile(file);
    });

    await waitFor(() => {
      expect(audioImportDataMock).toHaveBeenCalled();
    });
    expect(audioImportDataMock).toHaveBeenCalledWith(
      "session-1",
      [1, 2, 3],
      "drop.wav",
    );
    expect(audioImportMock).not.toHaveBeenCalled();
    expect(runBatchMock).toHaveBeenCalledWith(
      "/vault/sessions/session-1/audio.wav",
    );
    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });

  test.each(["webm", "aac"])(
    "imports pathless .%s drops without MIME",
    async (extension) => {
      const { result } = renderHook(() => useUploadFile("session-1"), {
        wrapper: createWrapper(),
      });
      const file = new File([new Uint8Array([1, 2, 3])], `drop.${extension}`, {
        type: "",
        lastModified: 1_700_000_000_000,
      });
      Object.defineProperty(file, "arrayBuffer", {
        value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
      });

      expect(isAudioUploadFile(file)).toBe(true);

      act(() => {
        result.current.processAudioFile(file);
      });

      await waitFor(() => {
        expect(audioImportDataMock).toHaveBeenCalled();
      });
      expect(audioImportDataMock).toHaveBeenCalledWith(
        "session-1",
        [1, 2, 3],
        `drop.${extension}`,
      );
    },
  );

  test("persists imported subtitles before enhancing", async () => {
    let resolveWrite: (() => void) | undefined;
    createTranscriptMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveWrite = resolve;
        }),
    );
    parseSubtitleMock.mockResolvedValue({
      status: "ok",
      data: {
        tokens: [{ text: "Hello", start_time: 0, end_time: 500 }],
      },
    });
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/session.vtt", "transcript");
    });

    await waitFor(() => {
      expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    });
    expect(enhanceMock).not.toHaveBeenCalled();

    await act(async () => {
      resolveWrite?.();
    });
    await waitFor(() => {
      expect(enhanceMock).toHaveBeenCalledWith("session-1");
    });
    expect(createTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "session-1",
        ownerUserId: "user-1",
        source: "subtitle_import",
        words: [
          expect.objectContaining({
            text: "Hello",
            start_ms: 0,
            end_ms: 500,
          }),
        ],
      }),
    );
    expect(createTranscriptMock.mock.invocationCallOrder[0]).toBeLessThan(
      enhanceMock.mock.invocationCallOrder[0],
    );
  });

  test.each(["mp3", "m4a", "webm", "aac", "flac", "ogg"])(
    "imports .%s paths through audioImport and runs the batch",
    async (extension) => {
      const { result } = renderHook(() => useUploadFile("session-1"), {
        wrapper: createWrapper(),
      });

      act(() => {
        result.current.processFile(`/tmp/recording.${extension}`, "audio");
      });

      await waitFor(() => {
        expect(runBatchMock).toHaveBeenCalledWith(
          "/vault/sessions/session-1/audio.mp3",
        );
      });
      expect(audioImportMock).toHaveBeenCalledWith(
        "session-1",
        `/tmp/recording.${extension}`,
      );
      expect(audioImportDataMock).not.toHaveBeenCalled();
    },
  );

  test("ignores paths with non-audio extensions", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/notes.txt", "audio");
    });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(audioImportMock).not.toHaveBeenCalled();
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("queues auto-enhancement after the imported batch completes", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/recording.mp3", "audio");
    });

    await waitFor(() => {
      expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
        "session-1",
      );
    });
    expect(runBatchMock.mock.invocationCallOrder[0]).toBeLessThan(
      queueAutoEnhanceIfSummaryEmptyMock.mock.invocationCallOrder[0],
    );
    expect(handleBatchStartedMock).toHaveBeenCalledWith(
      "session-1",
      "importing",
    );
    expect(clearBatchSessionMock).toHaveBeenCalledWith("session-1");
  });

  test("estimates the note date from audio metadata for path imports", async () => {
    audioSourceMetadataMock.mockResolvedValue({
      status: "ok",
      data: {
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: null,
        durationMs: 60_000,
      },
    });

    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/recording.m4a", "audio");
    });

    await waitFor(() => {
      expect(updateSessionMock).toHaveBeenCalledWith({
        created_at: "2026-03-26T11:59:00.000Z",
      });
    });
    // The date lands before the import starts so the session sorts correctly
    // while the batch is still running.
    expect(updateSessionMock.mock.invocationCallOrder[0]).toBeLessThan(
      audioImportMock.mock.invocationCallOrder[0],
    );
  });

  test("keeps the session date when it has calendar-event metadata", async () => {
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "",
      event_json: '{"id":"event-1"}',
    });
    audioSourceMetadataMock.mockResolvedValue({
      status: "ok",
      data: {
        createdAt: "2026-03-26T12:00:00.000Z",
        modifiedAt: null,
        durationMs: 60_000,
      },
    });

    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/recording.m4a", "audio");
    });

    await waitFor(() => {
      expect(runBatchMock).toHaveBeenCalled();
    });
    expect(updateSessionMock).not.toHaveBeenCalled();
  });

  test("uses the file's lastModified for pathless dropped audio", async () => {
    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });
    const lastModified = Date.parse("2026-03-26T10:00:00.000Z");
    const file = new File([new Uint8Array([1, 2, 3])], "drop.webm", {
      type: "audio/webm",
      lastModified,
    });
    Object.defineProperty(file, "arrayBuffer", {
      value: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3]).buffer),
    });

    act(() => {
      result.current.processAudioFile(file);
    });

    await waitFor(() => {
      expect(updateSessionMock).toHaveBeenCalledWith({
        created_at: "2026-03-26T10:00:00.000Z",
      });
    });
  });

  test("reports import failures to the listener store", async () => {
    audioImportMock.mockResolvedValue({
      status: "error",
      error: "decode failed",
    });

    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/recording.mp3", "audio");
    });

    await waitFor(() => {
      expect(handleBatchFailedMock).toHaveBeenCalledWith(
        "session-1",
        "decode failed",
      );
    });
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("forwards import progress events for this session", async () => {
    let emit:
      | ((event: {
          payload: {
            type: string;
            session_id: string;
            percentage: number;
          };
        }) => void)
      | undefined;
    audioImportListenMock.mockImplementation(async (listener) => {
      emit = listener;
      return vi.fn();
    });
    audioImportMock.mockImplementation(async () => {
      emit?.({
        payload: {
          type: "audioImportProgress",
          session_id: "session-1",
          percentage: 42,
        },
      });
      emit?.({
        payload: {
          type: "audioImportProgress",
          session_id: "other-session",
          percentage: 99,
        },
      });
      return { status: "ok", data: "/vault/sessions/session-1/audio.mp3" };
    });

    const { result } = renderHook(() => useUploadFile("session-1"), {
      wrapper: createWrapper(),
    });

    act(() => {
      result.current.processFile("/tmp/recording.mp3", "audio");
    });

    await waitFor(() => {
      expect(runBatchMock).toHaveBeenCalled();
    });
    expect(updateBatchProgressMock).toHaveBeenCalledWith("session-1", 42);
    expect(updateBatchProgressMock).not.toHaveBeenCalledWith(
      "session-1",
      99,
    );
  });
});
