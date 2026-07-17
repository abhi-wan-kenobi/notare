import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  useRegenerateTranscript,
  useRegenerateTranscriptWithModel,
} from "./actions";

const {
  askMock,
  audioPathMock,
  runBatchMock,
  handleBatchFailedMock,
  queueAutoEnhanceIfSummaryEmptyMock,
  showTransientToastMock,
  resolveFinalBatchTargetMock,
  useSTTConnectionMock,
} = vi.hoisted(() => ({
  askMock: vi.fn(),
  audioPathMock: vi.fn(),
  runBatchMock: vi.fn(),
  handleBatchFailedMock: vi.fn(),
  queueAutoEnhanceIfSummaryEmptyMock: vi.fn(),
  showTransientToastMock: vi.fn(),
  resolveFinalBatchTargetMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
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

vi.mock("~/stt/final-model", () => ({
  resolveFinalBatchTarget: resolveFinalBatchTargetMock,
}));

vi.mock("~/stt/useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
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

describe("useRegenerateTranscriptWithModel", () => {
  const restoreMock = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    askMock.mockResolvedValue(true);
    audioPathMock.mockResolvedValue({
      status: "ok",
      data: "/notes/session-1/audio.mp3",
    });
    runBatchMock.mockResolvedValue(undefined);
    queueAutoEnhanceIfSummaryEmptyMock.mockResolvedValue(undefined);
    restoreMock.mockResolvedValue(undefined);
    resolveFinalBatchTargetMock.mockResolvedValue({
      model: "QuantizedSmall",
      baseUrl: "http://127.0.0.1:6666",
      restore: restoreMock,
    });
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
  });

  test("runs the batch with the chosen model and restores the live server", async () => {
    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(askMock).toHaveBeenCalled();
    expect(resolveFinalBatchTargetMock).toHaveBeenCalledWith({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });
    expect(runBatchMock).toHaveBeenCalledWith("/notes/session-1/audio.mp3", {
      model: "QuantizedSmall",
      baseUrl: "http://127.0.0.1:6666",
      provider: "hyprnote",
    });
    expect(restoreMock).toHaveBeenCalledTimes(1);
    expect(queueAutoEnhanceIfSummaryEmptyMock).toHaveBeenCalledWith(
      "session-1",
    );
    expect(
      runBatchMock.mock.invocationCallOrder[0],
    ).toBeLessThan(restoreMock.mock.invocationCallOrder[0]);
  });

  test("passes no live model for restore when the live provider is external", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "key",
      },
    });

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(resolveFinalBatchTargetMock).toHaveBeenCalledWith({
      provider: "hyprnote",
      liveModel: null,
      finalModel: "QuantizedSmall",
    });
  });

  test("does nothing when the user cancels", async () => {
    askMock.mockResolvedValue(false);

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(audioPathMock).not.toHaveBeenCalled();
    expect(resolveFinalBatchTargetMock).not.toHaveBeenCalled();
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("shows a toast when the recording is missing", async () => {
    audioPathMock.mockResolvedValue({ status: "error", error: "not found" });

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(showTransientToastMock).toHaveBeenCalledWith(
      expect.objectContaining({ variant: "error" }),
    );
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("shows a toast when the model cannot be prepared", async () => {
    resolveFinalBatchTargetMock.mockResolvedValue(null);

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(showTransientToastMock).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "transcript-regenerate-model-unavailable-session-1",
        variant: "error",
      }),
    );
    expect(runBatchMock).not.toHaveBeenCalled();
  });

  test("reports batch failures and still restores the live server", async () => {
    runBatchMock.mockRejectedValue(new Error("model exploded"));

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(handleBatchFailedMock).toHaveBeenCalledWith(
      "session-1",
      "model exploded",
    );
    expect(restoreMock).toHaveBeenCalledTimes(1);
  });

  test("ignores intentional stop errors but restores the live server", async () => {
    runBatchMock.mockRejectedValue(new Error("Transcription stopped."));

    const { result } = renderHook(() =>
      useRegenerateTranscriptWithModel("session-1"),
    );

    await act(async () => {
      await result.current("QuantizedSmall");
    });

    expect(handleBatchFailedMock).not.toHaveBeenCalled();
    expect(restoreMock).toHaveBeenCalledTimes(1);
  });
});
