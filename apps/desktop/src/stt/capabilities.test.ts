import { beforeEach, describe, expect, test, vi } from "vitest";

const { isSupportedLanguagesBatchMock, isSupportedLanguagesLiveMock } =
  vi.hoisted(() => ({
    isSupportedLanguagesBatchMock: vi.fn(),
    isSupportedLanguagesLiveMock: vi.fn(),
  }));

vi.mock("@hypr/plugin-transcription", () => ({
  commands: {
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
    isSupportedLanguagesLive: isSupportedLanguagesLiveMock,
  },
}));

import {
  expandHinglish,
  getLiveTranscriptionConfig,
  getOnDeviceTranscriptionConfig,
  getOnDeviceTranscriptionMode,
  getTranscriptionLanguages,
  isConfiguredSttModel,
  isLiveTranscriptionSupported,
  isSupportedLanguagesBatch,
  isSupportedLanguagesLive,
  isSupportedLocalSttModel,
  isVoxtralLocalSttModel,
} from "./capabilities";

beforeEach(() => {
  vi.clearAllMocks();
  isSupportedLanguagesLiveMock.mockResolvedValue({
    status: "ok",
    data: true,
  });
  isSupportedLanguagesBatchMock.mockResolvedValue({
    status: "ok",
    data: true,
  });
});

describe("getOnDeviceTranscriptionMode", () => {
  test("uses live mode for realtime local models", () => {
    expect(getOnDeviceTranscriptionMode("soniqo-parakeet-streaming")).toBe(
      "live",
    );
  });

  test("uses batch mode for non-realtime local models", () => {
    expect(getOnDeviceTranscriptionMode("soniqo-qwen3-small")).toBe("batch");
  });

  test("uses live mode for local whisper models", () => {
    expect(getOnDeviceTranscriptionMode("QuantizedSmall")).toBe("live");
    expect(getOnDeviceTranscriptionMode("QuantizedTinyEn")).toBe("live");
    expect(getOnDeviceTranscriptionMode("QuantizedLargeTurbo")).toBe("live");
  });

  test("keeps am models batch", () => {
    expect(getOnDeviceTranscriptionMode("am-parakeet-v3")).toBe("batch");
  });

  test("keeps live mode when realtime local model has no Soniqo-supported language", () => {
    expect(
      getOnDeviceTranscriptionMode("soniqo-parakeet-streaming", ["ko"]),
    ).toBe("live");
  });

  test("keeps European Soniqo streaming languages live", () => {
    expect(
      getOnDeviceTranscriptionMode("soniqo-parakeet-streaming", ["de"]),
    ).toBe("live");
  });

  test("uses batch mode for the Voxtral (llama.cpp) model — no streaming decode", () => {
    expect(getOnDeviceTranscriptionMode("voxtral-mini-3b-2507-q4km")).toBe(
      "batch",
    );
  });
});

describe("isSupportedLocalSttModel", () => {
  test("accepts shipped local STT model families", () => {
    expect(isSupportedLocalSttModel("soniqo-parakeet-streaming")).toBe(true);
    expect(isSupportedLocalSttModel("am-parakeet-v3")).toBe(true);
    expect(isSupportedLocalSttModel("QuantizedSmallEn")).toBe(true);
    expect(isSupportedLocalSttModel("parakeet-tdt-v3-int8")).toBe(true);
    expect(isSupportedLocalSttModel("voxtral-mini-3b-2507-q4km")).toBe(true);
  });

  test("rejects cloud, local LLM, and removed local model ids", () => {
    expect(isSupportedLocalSttModel("cloud")).toBe(false);
    expect(isSupportedLocalSttModel("Llama3p2_3bQ4")).toBe(false);
    expect(isSupportedLocalSttModel("removed-local-model")).toBe(false);
  });
});

describe("isVoxtralLocalSttModel", () => {
  test("recognizes the Voxtral (llama.cpp) model id prefix", () => {
    expect(isVoxtralLocalSttModel("voxtral-mini-3b-2507-q4km")).toBe(true);
    expect(isVoxtralLocalSttModel("parakeet-tdt-v3-int8")).toBe(false);
    expect(isVoxtralLocalSttModel(undefined)).toBe(false);
  });
});

describe("isConfiguredSttModel", () => {
  test("requires known model ids for Notare STT", () => {
    expect(isConfiguredSttModel("hyprnote", "cloud")).toBe(true);
    expect(isConfiguredSttModel("hyprnote", "soniqo-qwen3-small")).toBe(true);
    expect(isConfiguredSttModel("hyprnote", "voxtral-mini-3b-2507-q4km")).toBe(
      true,
    );
    expect(isConfiguredSttModel("hyprnote", "removed-local-model")).toBe(false);
  });

  test("allows custom model ids for external providers", () => {
    expect(isConfiguredSttModel("custom", "whisper-large-v3")).toBe(true);
  });
});

describe("getOnDeviceTranscriptionConfig", () => {
  test("uses the first supported language for realtime local models", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["en", "ko"]),
    ).toEqual({
      languages: ["en"],
      transcriptionMode: "live",
    });
  });

  test("keeps German live even when English is an additional language", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["de", "en"]),
    ).toEqual({
      languages: ["de"],
      transcriptionMode: "live",
    });
  });

  test("drops unsupported Soniqo language hints instead of forcing batch", () => {
    expect(
      getOnDeviceTranscriptionConfig("soniqo-parakeet-streaming", ["ko"]),
    ).toEqual({
      languages: [],
      transcriptionMode: "live",
    });
  });

  test("keeps all languages live for local whisper models", () => {
    expect(
      getOnDeviceTranscriptionConfig("QuantizedSmall", ["en", "hi"]),
    ).toEqual({
      languages: ["en", "hi"],
      transcriptionMode: "live",
    });
  });
});

describe("getLiveTranscriptionConfig", () => {
  test("keeps all languages when the selected provider supports them live", async () => {
    const config = await getLiveTranscriptionConfig({
      provider: "deepgram",
      model: "nova-3-general",
      languages: ["en", "es"],
    });

    expect(config).toEqual({
      languages: ["en", "es"],
      transcriptionMode: undefined,
    });
    expect(isSupportedLanguagesLiveMock).toHaveBeenCalledTimes(1);
  });

  test("falls back to the main language when additional languages are unsupported live", async () => {
    isSupportedLanguagesLiveMock.mockImplementation(
      (_provider, _model, languages) =>
        Promise.resolve({
          status: "ok",
          data: languages.length === 1 && languages[0] === "en",
        }),
    );

    await expect(
      getLiveTranscriptionConfig({
        provider: "deepgram",
        model: "nova-3-general",
        languages: ["en", "ko"],
      }),
    ).resolves.toEqual({
      languages: ["en"],
      transcriptionMode: undefined,
    });
  });

  test("checks custom providers as Deepgram-compatible for language fallback", async () => {
    isSupportedLanguagesLiveMock.mockImplementation(
      (_provider, _model, languages) =>
        Promise.resolve({
          status: "ok",
          data: languages.length === 1 && languages[0] === "en",
        }),
    );

    await getLiveTranscriptionConfig({
      provider: "custom",
      model: "nova-3-general",
      languages: ["en", "ko"],
    });

    expect(isSupportedLanguagesLiveMock.mock.calls[0]?.[0]).toBe("deepgram");
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for language fallback", async () => {
    await getLiveTranscriptionConfig({
      provider: "cloudflare_workers_ai",
      model: "nova-3",
      languages: ["en", "ko"],
    });

    expect(isSupportedLanguagesLiveMock.mock.calls[0]?.[0]).toBe("deepgram");
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for live language support", async () => {
    await isSupportedLanguagesLive("cloudflare_workers_ai", "nova-3", ["en"]);

    expect(isSupportedLanguagesLiveMock.mock.calls[0]).toEqual([
      "deepgram",
      "nova-3",
      ["en"],
    ]);
  });

  test("checks Cloudflare Workers AI as Deepgram-compatible for batch language support", async () => {
    await isSupportedLanguagesBatch("cloudflare_workers_ai", "nova-3", ["en"]);

    expect(isSupportedLanguagesBatchMock.mock.calls[0]).toEqual([
      "deepgram",
      "nova-3",
      ["en"],
    ]);
  });
});

describe("getTranscriptionLanguages", () => {
  test("prefers the main language before additional spoken languages", () => {
    expect(getTranscriptionLanguages("en", ["ko"])).toEqual(["en", "ko"]);
  });

  test("deduplicates regional variants by base language", () => {
    expect(getTranscriptionLanguages("en-US", ["en", "ko"])).toEqual([
      "en-US",
      "ko",
    ]);
  });
});

describe("isLiveTranscriptionSupported", () => {
  test("reports local whisper/parakeet models as live-capable", async () => {
    isSupportedLanguagesLiveMock.mockResolvedValue({
      status: "ok",
      data: true,
    });

    expect(
      await isLiveTranscriptionSupported("hyprnote", "QuantizedSmall"),
    ).toBe(true);
    expect(
      await isLiveTranscriptionSupported("hyprnote", "parakeet-tdt-0.6b-v3"),
    ).toBe(true);
  });

  test("never reports custom/remote/cloud providers as live-capable", async () => {
    // Remote, custom and cloud providers are batch-only — even though custom
    // speaks a Deepgram-compatible protocol, live runs on the local loopback
    // server only. The listener command must not be consulted for them.
    isSupportedLanguagesLiveMock.mockResolvedValue({
      status: "ok",
      data: true,
    });

    expect(await isLiveTranscriptionSupported("custom", "nova-3")).toBe(false);
    expect(await isLiveTranscriptionSupported("deepgram", "nova-3")).toBe(
      false,
    );
    expect(await isLiveTranscriptionSupported("hyprnote", "cloud")).toBe(false);
    expect(await isLiveTranscriptionSupported("", "")).toBe(false);

    expect(isSupportedLanguagesLiveMock).not.toHaveBeenCalled();
  });
});

describe("expandHinglish", () => {
  test("is a no-op when the sentinel is absent", () => {
    expect(
      expandHinglish(["en"], { model: "voxtral-mini-3b-2507-q4km" }),
    ).toEqual(["en"]);
    expect(
      expandHinglish([], { provider: "custom", model: "Quantized-x" }),
    ).toEqual([]);
  });

  test("Voxtral gets hi,en (its promptable code-mix mode)", () => {
    expect(
      expandHinglish(["hinglish"], { model: "voxtral-mini-3b-2507-q4km" }),
    ).toEqual(["hi", "en"]);
  });

  test("Whisper (bundled or remote custom server) gets en", () => {
    expect(
      expandHinglish(["hinglish"], { model: "QuantizedLargeTurbo" }),
    ).toEqual(["en"]);
    expect(
      expandHinglish(["hinglish"], {
        provider: "custom",
        model: "QuantizedTiny",
      }),
    ).toEqual(["en"]);
  });

  test("merges with other selected languages and de-dupes", () => {
    expect(
      expandHinglish(["hinglish", "en"], {
        model: "voxtral-mini-3b-2507-q4km",
      }),
    ).toEqual(["hi", "en"]);
    expect(
      expandHinglish(["fr", "hinglish"], {
        model: "voxtral-mini-3b-2507-q4km",
      }),
    ).toEqual(["fr", "hi", "en"]);
  });
});
