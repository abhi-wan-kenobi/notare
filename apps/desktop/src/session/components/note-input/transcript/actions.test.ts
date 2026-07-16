import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { useRegenerateTranscript } from "./actions";

const {
  askMock,
  audioPathMock,
  runBatchMock,
  handleBatchFailedMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  showTransientToastMock,
} = vi.hoisted(() => ({
  askMock: vi.fn(),
  audioPathMock: vi.fn(),
  runBatchMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  showTransientToastMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: askMock,
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    audioPath: audioPathMock,
  },
}));

vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({
    queueAutoEnhanceIfSummaryEmpty: queueAutoEnhanceIfSummaryEmptyMock,
  }),
}));

vi.mock("~/sidebar/toast/transient", () => ({
  showTransientToast: showTransientToastMock,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({ handleBatchFailed: handleBatchFailedMock }),
}));

vi.mock("~/stt/useRunBatch", () => ({
  useRunBatch: () => runBatchMock,
  isStoppedTranscriptionError: (error: unknown) =>
    (error instanceof Error ? error.message : String(error)) ===
    "Transcription stopped.",
}));

describe("useRegenerateTranscript", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    askMock.mockResolvedValue(true);
    audioPathMock.mockResolvedValue({
      status: "ok",
      data: "/notes/session-1/audio.wav",
    });
    runBatchMock.mockResolvedValue(undefined);
    queueAutoEnhanceIfSummaryEmptyMock.mockResolvedValue(undefined);
  });

  test("asks for confirmation before replacing the transcript", async () => {
    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(askMock).toHaveBeenCalledWith(
      "Re-transcribe this recording? The current transcript will be replaced.",
      expect.objectContaining({ title: "Re-transcribe recording" }),
    );
    expect(runBatchMock).toHaveBeenCalledWith("/notes/session-1/audio.wav");
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
  });

  test("does nothing when the user cancels", async () => {
    askMock.mockResolvedValue(false);

    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(audioPathMock).not.toHaveBeenCalled();
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("shows a toast when the recording is missing", async () => {
    audioPathMock.mockResolvedValue({ status: "error", error: "not found" });

    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(showTransientToastMock).toHaveBeenCalledWith(
      expect.objectContaining({ variant: "error" }),
    );
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("reports batch failures to the listener store", async () => {
    runBatchMock.mockRejectedValue(new Error("model exploded"));

    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(handleBatchFailedMock).toHaveBeenCalledWith(
      "session-1",
      "model exploded",
    );
  });

  test("ignores intentional stop errors", async () => {
    runBatchMock.mockRejectedValue(new Error("Transcription stopped."));

    const { result } = renderHook(() => useRegenerateTranscript("session-1"));

    await act(async () => {
      await result.current();
    });

    expect(handleBatchFailedMock).not.toHaveBeenCalled();
  });
});
