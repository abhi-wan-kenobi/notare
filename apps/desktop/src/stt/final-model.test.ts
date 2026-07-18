import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  listDownloadedFinalSttModels,
  pickFinalSttModel,
  resolveFinalBatchTarget,
} from "./final-model";

const {
  isModelDownloadedMock,
  startServerMock,
  stopServerMock,
  listSupportedModelsMock,
} = vi.hoisted(() => ({
  isModelDownloadedMock: vi.fn(),
  startServerMock: vi.fn(),
  stopServerMock: vi.fn(),
  listSupportedModelsMock: vi.fn(),
}));

vi.mock("@hypr/plugin-local-stt", () => ({
  commands: {
    isModelDownloaded: isModelDownloadedMock,
    startServer: startServerMock,
    stopServer: stopServerMock,
    listSupportedModels: listSupportedModelsMock,
  },
}));

function modelInfo(
  key: string,
  recommendedUse: "live" | "final" | "liveAndFinal",
  displayName = key,
) {
  return {
    key,
    display_name: displayName,
    description: "",
    size_bytes: null,
    model_type: "whispercpp",
    engine: "whisper.cpp",
    languages: "multilingual",
    language_count: null,
    tier: "balanced",
    recommended_use: recommendedUse,
  };
}

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
    listSupportedModelsMock.mockResolvedValue({ status: "ok", data: [] });
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

  test("carries the configured base URL/API key for a custom batch provider", async () => {
    // Batch may target a custom server independent of the local live model;
    // the caller supplies the provider's configured base URL/API key.
    const target = await resolveFinalBatchTarget({
      provider: "custom",
      liveModel: "QuantizedTiny",
      finalModel: "whisper-large-v3",
      finalBaseUrl: "https://custom.test",
      finalApiKey: "custom-key",
    });

    expect(target).not.toBeNull();
    expect(target!.model).toBe("whisper-large-v3");
    expect(target!.baseUrl).toBe("https://custom.test");
    expect(target!.apiKey).toBe("custom-key");

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

  test("accepts non-prefix local models from the supported-model catalog", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "ok",
      data: [modelInfo("parakeet-v3", "final")],
    });

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "parakeet-v3",
    });

    expect(target).not.toBeNull();
    expect(target!.model).toBe("parakeet-v3");
    expect(startServerMock).toHaveBeenCalledWith("parakeet-v3");
  });

  test("restore restarts a non-prefix live model listed in the catalog", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "ok",
      data: [modelInfo("parakeet-v3", "liveAndFinal")],
    });

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "parakeet-v3",
      finalModel: "QuantizedSmall",
    });

    expect(target).not.toBeNull();

    await target!.restore();
    expect(startServerMock).toHaveBeenLastCalledWith("parakeet-v3");
    expect(stopServerMock).not.toHaveBeenCalled();
  });

  test("rejects models that are neither prefix-known nor in the catalog", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "ok",
      data: [modelInfo("parakeet-v3", "final")],
    });

    const target = await resolveFinalBatchTarget({
      provider: "hyprnote",
      liveModel: "QuantizedTiny",
      finalModel: "nova-3",
    });

    expect(target).toBeNull();
    expect(isModelDownloadedMock).not.toHaveBeenCalled();
  });
});

describe("listDownloadedFinalSttModels", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test("returns downloaded models recommended for final transcription", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "ok",
      data: [
        modelInfo("soniqo-parakeet-streaming", "live"),
        modelInfo("QuantizedSmall", "final", "Whisper Small"),
        modelInfo("QuantizedLargeTurbo", "final", "Whisper Large Turbo"),
        modelInfo("am-parakeet-v3", "liveAndFinal", "Parakeet v3"),
      ],
    });
    isModelDownloadedMock.mockImplementation(async (model: string) => ({
      status: "ok",
      data: model !== "QuantizedLargeTurbo",
    }));

    await expect(listDownloadedFinalSttModels()).resolves.toEqual([
      { key: "QuantizedSmall", displayName: "Whisper Small" },
      { key: "am-parakeet-v3", displayName: "Parakeet v3" },
    ]);
    expect(isModelDownloadedMock).not.toHaveBeenCalledWith(
      "soniqo-parakeet-streaming",
    );
  });

  test("returns an empty list when the catalog is unavailable", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "error",
      error: "no plugin",
    });
    await expect(listDownloadedFinalSttModels()).resolves.toEqual([]);

    listSupportedModelsMock.mockRejectedValue(new Error("ipc down"));
    await expect(listDownloadedFinalSttModels()).resolves.toEqual([]);
  });

  test("skips models whose download check fails", async () => {
    listSupportedModelsMock.mockResolvedValue({
      status: "ok",
      data: [modelInfo("QuantizedSmall", "final")],
    });
    isModelDownloadedMock.mockRejectedValue(new Error("ipc down"));

    await expect(listDownloadedFinalSttModels()).resolves.toEqual([]);
  });
});
