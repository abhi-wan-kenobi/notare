import { beforeEach, describe, expect, test, vi } from "vitest";

import { pickFinalSttModel, resolveFinalBatchTarget } from "./final-model";

const { isModelDownloadedMock, startServerMock, stopServerMock } = vi.hoisted(
  () => ({
    isModelDownloadedMock: vi.fn(),
    startServerMock: vi.fn(),
    stopServerMock: vi.fn(),
  }),
);

vi.mock("@hypr/plugin-local-stt", () => ({
  commands: {
    isModelDownloaded: isModelDownloadedMock,
    startServer: startServerMock,
    stopServer: stopServerMock,
  },
}));

describe("pickFinalSttModel", () => {
  test("returns null when no final model is configured", () => {
    expect(
      pickFinalSttModel({ finalSetting: undefined, liveModel: "nova-3" }),
    ).toBeNull();
    expect(
      pickFinalSttModel({ finalSetting: "", liveModel: "nova-3" }),
    ).toBeNull();
    expect(
      pickFinalSttModel({ finalSetting: "   ", liveModel: "nova-3" }),
    ).toBeNull();
  });

  test("ignores non-string values", () => {
    expect(
      pickFinalSttModel({ finalSetting: [], liveModel: "nova-3" }),
    ).toBeNull();
    expect(
      pickFinalSttModel({ finalSetting: 42, liveModel: "nova-3" }),
    ).toBeNull();
  });

  test("returns null when the final model equals the live model", () => {
    expect(
      pickFinalSttModel({ finalSetting: "nova-3", liveModel: "nova-3" }),
    ).toBeNull();
  });

  test("returns the final model when it differs from the live model", () => {
    expect(
      pickFinalSttModel({
        finalSetting: "QuantizedSmall",
        liveModel: "QuantizedTiny",
      }),
    ).toBe("QuantizedSmall");
    expect(
      pickFinalSttModel({ finalSetting: "QuantizedSmall", liveModel: null }),
    ).toBe("QuantizedSmall");
  });
});

describe("resolveFinalBatchTarget", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isModelDownloadedMock.mockResolvedValue({ status: "ok", data: true });
    startServerMock.mockResolvedValue({
      status: "ok",
      data: "http://127.0.0.1:6666",
    });
    stopServerMock.mockResolvedValue({ status: "ok", data: true });
  });

  test("external providers reuse the live connection with a new model id", async () => {
    const target = await resolveFinalBatchTarget({
      provider: "deepgram",
      liveModel: "nova-3",
      finalModel: "nova-2-medical",
    });

    expect(target).not.toBeNull();
    expect(target!.model).toBe("nova-2-medical");
    expect(target!.baseUrl).toBeUndefined();

    await target!.restore();
    expect(startServerMock).not.toHaveBeenCalled();
    expect(stopServerMock).not.toHaveBeenCalled();
  });

  test("rejects non-local model ids for the local provider", async () => {
    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "cloud",
    });

    expect(target).toBeNull();
    expect(isModelDownloadedMock).not.toHaveBeenCalled();
  });

  test("returns null when the local final model is not downloaded", async () => {
    isModelDownloadedMock.mockResolvedValue({ status: "ok", data: false });

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });

    expect(target).toBeNull();
    expect(startServerMock).not.toHaveBeenCalled();
  });

  test("returns null when the local server fails to start", async () => {
    startServerMock.mockResolvedValue({ status: "error", error: "boom" });

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });

    expect(target).toBeNull();
  });

  test("returns null when plugin commands throw", async () => {
    isModelDownloadedMock.mockRejectedValue(new Error("ipc down"));

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });

    expect(target).toBeNull();
  });

  test("starts the local server for the final model and restores the live one", async () => {
    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });

    expect(target).not.toBeNull();
    expect(target!.model).toBe("QuantizedSmall");
    expect(target!.baseUrl).toBe("http://127.0.0.1:6666");
    expect(startServerMock).toHaveBeenCalledWith("QuantizedSmall");

    await target!.restore();
    expect(startServerMock).toHaveBeenLastCalledWith("QuantizedTiny");
    expect(stopServerMock).not.toHaveBeenCalled();
  });

  test("restore stops the server when the live model is not local", async () => {
    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: null,
      finalModel: "QuantizedSmall",
    });

    expect(target).not.toBeNull();

    await target!.restore();
    expect(stopServerMock).toHaveBeenCalledWith(null);
  });

  test("restore swallows errors", async () => {
    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "QuantizedSmall",
    });
    startServerMock.mockRejectedValue(new Error("restart failed"));

    await expect(target!.restore()).resolves.toBeUndefined();
  });
});
