import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  canRunBatchTranscription,
  getBatchFallbackTarget,
  getBatchProvider,
  getLocalBatchFallbackFromConn,
  getSessionSpeakerCount,
  NO_BATCH_PROVIDER_ERROR,
} from "./useRunBatch";
import { useRunBatch } from "./useRunBatch";

const {
  startTranscriptionMock,
  useListenerMock,
  useSessionMock,
  useSessionParticipantsMock,
  useSTTConnectionMock,
  useAuthMock,
  useBillingAccessMock,
  useConfigValueMock,
  listSupportedModelsMock,
  isSupportedLanguagesBatchMock,
  sonnerToastMessageMock,
  deleteProcessedAudioForRetentionMock,
  createTranscriptMock,
  appendTranscriptWordsAndHintsMock,
  idMock,
  isModelDownloadedMock,
  startServerMock,
  stopServerMock,
  platformMock,
  useAiProviderMock,
} = vi.hoisted(() => ({
  startTranscriptionMock: vi.fn(),
  useListenerMock: vi.fn(),
  useSessionMock: vi.fn(),
  useSessionParticipantsMock: vi.fn(),
  useSTTConnectionMock: vi.fn(),
  useAuthMock: vi.fn(),
  useBillingAccessMock: vi.fn(),
  useConfigValueMock: vi.fn(),
  isSupportedLanguagesBatchMock: vi.fn(),
  sonnerToastMessageMock: vi.fn(),
  deleteProcessedAudioForRetentionMock: vi.fn(),
  createTranscriptMock: vi.fn(),
  appendTranscriptWordsAndHintsMock: vi.fn(),
  idMock: vi.fn(),
  isModelDownloadedMock: vi.fn(),
  startServerMock: vi.fn(),
  stopServerMock: vi.fn(),
  listSupportedModelsMock: vi.fn(),
  platformMock: { current: "macos" as string },
  useAiProviderMock: vi.fn(),
}));

vi.mock("@hypr/plugin-local-stt", () => ({
  commands: {
    isModelDownloaded: isModelDownloadedMock,
    startServer: startServerMock,
    stopServer: stopServerMock,
    listSupportedModels: listSupportedModelsMock,
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => platformMock.current,
}));

vi.mock("~/settings/providers", () => ({
  useAiProvider: useAiProviderMock,
}));

vi.mock("./contexts", () => ({
  useListener: useListenerMock,
}));

vi.mock("./useKeywords", () => ({
  getSessionKeywords: vi.fn(async () => []),
  useKeywords: vi.fn(() => []),
}));

vi.mock("./useSTTConnection", () => ({
  useSTTConnection: useSTTConnectionMock,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    message: sonnerToastMessageMock,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: useAuthMock,
}));

vi.mock("~/auth/billing", () => ({
  useBillingAccess: useBillingAccessMock,
}));

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.test",
  },
}));

vi.mock("~/services/audio-retention", () => ({
  deleteProcessedAudioForRetention: deleteProcessedAudioForRetentionMock,
  normalizeAudioRetention: (value: unknown) =>
    typeof value === "string" ? value : "forever",
}));

vi.mock("~/session/queries", () => ({
  useSession: useSessionMock,
  useSessionParticipants: useSessionParticipantsMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: useConfigValueMock,
}));

vi.mock("~/shared/utils", () => ({
  id: idMock,
}));

vi.mock("~/stt/capabilities", () => {
  const baseLanguageCode = (language: string) =>
    language.split(/[-_]/)[0]?.toLowerCase() ?? "";

  const isSupportedLocalSttModel = (model?: string | null) =>
    typeof model === "string" &&
    (model.startsWith("soniqo-") ||
      model.startsWith("am-") ||
      model.startsWith("Quantized"));

  const isWhisperLocalSttModel = (model?: string | null) =>
    typeof model === "string" && model.startsWith("Quantized");

  const isParakeetLocalSttModel = (model?: string | null) =>
    typeof model === "string" && model.startsWith("parakeet-");

  return {
    isSupportedLocalSttModel,
    isWhisperLocalSttModel,
    isParakeetLocalSttModel,
    shouldRunLocalDiarization: (
      provider?: string | null,
      model?: string | null,
    ) =>
      isWhisperLocalSttModel(model) ||
      isParakeetLocalSttModel(model) ||
      provider === "custom",
    isHyprnoteLocalSttModel: (
      provider?: string | null,
      model?: string | null,
    ) => provider === "hyprnote" && isSupportedLocalSttModel(model),
    getTranscriptionLanguages: (
      mainLanguage: string | null | undefined,
      spokenLanguages: readonly string[] | null | undefined,
    ) => {
      const seen = new Set<string>();
      const languages: string[] = [];

      for (const language of [mainLanguage, ...(spokenLanguages ?? [])]) {
        if (!language) {
          continue;
        }

        const baseCode = baseLanguageCode(language);
        if (!baseCode || seen.has(baseCode)) {
          continue;
        }

        seen.add(baseCode);
        languages.push(language);
      }

      return languages;
    },
    expandHinglish: (
      languages: string[],
      engine: { provider?: string | null; model?: string | null },
    ) => {
      if (!languages.includes("hinglish")) {
        return languages;
      }
      const replacement =
        typeof engine.model === "string" && engine.model.startsWith("voxtral-")
          ? ["hi", "en"]
          : ["en"];
      const out: string[] = [];
      for (const language of languages) {
        for (const code of language === "hinglish" ? replacement : [language]) {
          if (!out.includes(code)) {
            out.push(code);
          }
        }
      }
      return out;
    },
    isSupportedLanguagesBatch: isSupportedLanguagesBatchMock,
  };
});

vi.mock("~/stt/queries", () => ({
  appendTranscriptWordsAndHints: appendTranscriptWordsAndHintsMock,
  createTranscript: createTranscriptMock,
}));

describe("getBatchProvider", () => {
  test("maps pyannote to the batch transcription provider", () => {
    expect(getBatchProvider("pyannote", "parakeet-tdt-0.6b-v3")).toBe(
      "pyannote",
    );
  });

  test("keeps openai mapped to the batch transcription provider", () => {
    expect(getBatchProvider("openai", "gpt-4o-transcribe")).toBe("openai");
  });

  test("keeps cartesia mapped to the batch transcription provider", () => {
    expect(getBatchProvider("cartesia", "ink-2")).toBe("cartesia");
  });

  test("maps Cloudflare Workers AI to the Deepgram-compatible batch provider", () => {
    expect(getBatchProvider("cloudflare_workers_ai", "nova-3")).toBe(
      "deepgram",
    );
  });

  test("maps local soniqo models to soniqo batch provider", () => {
    expect(getBatchProvider("hyprnote", "soniqo-parakeet-batch")).toBe(
      "soniqo",
    );
  });

  test("routes the custom server through the hyprnote batch adapter", () => {
    // The self-hosted companion server speaks the same /v1/listen protocol
    // as the local hyprnote server, so custom routes through the hyprnote
    // adapter — carrying the custom base URL/API key on the BatchTarget.
    expect(getBatchProvider("custom", "whisper-large-v3")).toBe("hyprnote");
  });
});

describe("getLocalBatchFallbackFromConn", () => {
  test("uses the live whisper connection as a batch fallback", () => {
    expect(
      getLocalBatchFallbackFromConn({
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
      }),
    ).toEqual({
      provider: "hyprnote",
      model: "QuantizedTiny",
      baseUrl: "http://127.0.0.1:5555",
      apiKey: "",
      label: "Local transcription",
    });
  });

  test("returns null for a non-local live connection", () => {
    expect(
      getLocalBatchFallbackFromConn({
        provider: "custom",
        model: "whisper-large-v3",
        baseUrl: "https://custom.test",
      }),
    ).toBeNull();
    expect(getLocalBatchFallbackFromConn(null)).toBeNull();
  });
});

describe("canRunBatchTranscription", () => {
  test("allows post-capture batch so useRunBatch can choose a fallback", () => {
    expect(canRunBatchTranscription(null)).toBe(true);
    expect(
      canRunBatchTranscription({
        provider: "custom",
        model: "realtime-only",
      }),
    ).toBe(true);
  });
});

describe("getBatchFallbackTarget", () => {
  test("uses hosted cloud transcription for paid users with a session", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: true,
        accessToken: "token",
        apiBaseUrl: "https://api.test",
      }),
    ).toEqual({
      provider: "hyprnote",
      model: "cloud",
      baseUrl: "https://api.test/stt",
      apiKey: "token",
      label: "Pro cloud transcription",
    });
  });

  test("uses local Soniqo batch transcription otherwise", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: false,
        accessToken: null,
        apiBaseUrl: "https://api.test",
      }),
    ).toEqual({
      provider: "soniqo",
      model: "soniqo-parakeet-batch",
      baseUrl: "soniqo://local",
      apiKey: "",
      label: "Soniqo batch transcription",
    });
  });

  test("uses the live local model on Windows/Linux when one is available", () => {
    const localFallback = {
      provider: "hyprnote" as const,
      model: "QuantizedTiny",
      baseUrl: "http://127.0.0.1:5555",
      apiKey: "",
      label: "Local transcription",
    };

    expect(
      getBatchFallbackTarget({
        isPaid: false,
        accessToken: null,
        apiBaseUrl: "https://api.test",
        platform: "windows",
        localBatchFallback: localFallback,
      }),
    ).toEqual(localFallback);
  });

  test("returns null on Windows/Linux when no local batch model is available", () => {
    expect(
      getBatchFallbackTarget({
        isPaid: false,
        accessToken: null,
        apiBaseUrl: "https://api.test",
        platform: "linux",
        localBatchFallback: null,
      }),
    ).toBeNull();
  });
});

describe("useRunBatch", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    let nextId = 0;
    idMock.mockImplementation(() => `generated-${++nextId}`);
    createTranscriptMock.mockResolvedValue(undefined);
    appendTranscriptWordsAndHintsMock.mockResolvedValue(undefined);
    deleteProcessedAudioForRetentionMock.mockResolvedValue(undefined);
    isSupportedLanguagesBatchMock.mockResolvedValue(true);
    useListenerMock.mockImplementation((selector) =>
      selector({ startTranscription: startTranscriptionMock }),
    );
    useSessionMock.mockReturnValue({
      id: "session-1",
      user_id: "user-1",
      raw_md: "Existing memo",
    });
    useSessionParticipantsMock.mockReturnValue([]);
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "test-key",
      },
    });
    useAuthMock.mockReturnValue({
      session: {
        access_token: "paid-token",
        user: { id: "user-1" },
      },
    });
    useBillingAccessMock.mockReturnValue({
      isPaid: false,
    });
    isModelDownloadedMock.mockResolvedValue({ status: "ok", data: true });
    startServerMock.mockResolvedValue({
      status: "ok",
      data: "http://127.0.0.1:6666",
    });
    stopServerMock.mockResolvedValue({ status: "ok", data: true });
    listSupportedModelsMock.mockResolvedValue({ status: "ok", data: [] });
    platformMock.current = "macos";
    useAiProviderMock.mockReturnValue(undefined);
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? ""
          : key === "final_stt_provider"
            ? ""
            : [],
    );
  });

  test("waits for streamed SQLite persists before retention", async () => {
    let resolveAppend: (() => void) | undefined;
    appendTranscriptWordsAndHintsMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveAppend = resolve;
        }),
    );
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "hello", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      options.handlePersist(
        [{ text: "world", start_ms: 100, end_ms: 200, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));
    const run = result.current("/tmp/session.wav");

    await waitFor(() => {
      expect(appendTranscriptWordsAndHintsMock).toHaveBeenCalledTimes(1);
    });
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();

    resolveAppend?.();
    await act(async () => await run);

    expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).toHaveBeenCalledTimes(1);
    expect(
      appendTranscriptWordsAndHintsMock.mock.invocationCallOrder[0],
    ).toBeLessThan(
      deleteProcessedAudioForRetentionMock.mock.invocationCallOrder[0],
    );
  });

  test("does not save for custom batch persist handlers", async () => {
    const handlePersist = vi.fn();
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "custom", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", { handlePersist });
    });

    expect(handlePersist).toHaveBeenCalledTimes(1);
    expect(createTranscriptMock).not.toHaveBeenCalled();
    expect(appendTranscriptWordsAndHintsMock).not.toHaveBeenCalled();
  });

  test("flushes default batch persists before rethrowing transcription errors", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "partial", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
      throw new Error("provider failed");
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow("provider failed");

    expect(createTranscriptMock).toHaveBeenCalledTimes(1);
    expect(deleteProcessedAudioForRetentionMock).not.toHaveBeenCalled();
  });

  test("passes selected transcription languages to batch transcription", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "soniqo-parakeet-batch",
        baseUrl: "soniqo://local",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "de" : ["en"],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        languages: ["de", "en"],
      }),
      expect.any(Object),
    );
  });

  test("routes a custom server through the hyprnote batch adapter with its base URL", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "custom",
        model: "realtime-only",
        baseUrl: "https://custom.test",
        apiKey: "custom-key",
      },
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    // custom speaks /v1/listen → hyprnote adapter, carrying the custom base
    // URL/API key (no fallback to Soniqo).
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "realtime-only",
        base_url: "https://custom.test",
        api_key: "custom-key",
      }),
      expect.any(Object),
    );
    expect(sonnerToastMessageMock).not.toHaveBeenCalled();
  });

  test("falls back to local Soniqo on macOS when the selected target's languages are unsupported", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "deepgram",
        model: "nova-3",
        baseUrl: "https://api.deepgram.com/v1/listen",
        apiKey: "test-key",
      },
    });
    isSupportedLanguagesBatchMock.mockResolvedValue(false);
    platformMock.current = "macos";
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "soniqo",
        model: "soniqo-parakeet-batch",
        base_url: "soniqo://local",
        api_key: "",
      }),
      expect.any(Object),
    );
  });

  test("falls back to the live local model on Windows when the selected target is unsupported", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
    isSupportedLanguagesBatchMock.mockResolvedValue(false);
    platformMock.current = "windows";
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "QuantizedTiny",
        base_url: "http://127.0.0.1:5555",
        api_key: "",
      }),
      expect.any(Object),
    );
  });

  test("throws a clear error when no batch target is available on Windows", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "custom",
        model: "realtime-only",
        baseUrl: "https://custom.test",
        apiKey: "custom-key",
      },
    });
    isSupportedLanguagesBatchMock.mockResolvedValue(false);
    platformMock.current = "windows";
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow(NO_BATCH_PROVIDER_ERROR);

    expect(startTranscriptionMock).not.toHaveBeenCalled();
  });

  test("runs batch on a custom server while live stays local (decoupled final provider)", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key: string) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "whisper-large-v3"
          : key === "final_stt_provider"
            ? "custom"
            : [],
    );
    useAiProviderMock.mockReturnValue({
      base_url: "https://custom.test",
      api_key: "custom-key",
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "whisper-large-v3",
        base_url: "https://custom.test",
        api_key: "custom-key",
      }),
      expect.any(Object),
    );
    // Batch targets a remote server; the live local server is left untouched.
    expect(startServerMock).not.toHaveBeenCalled();
    expect(stopServerMock).not.toHaveBeenCalled();
  });

  test("falls back to hosted cloud transcription for paid users", async () => {
    isSupportedLanguagesBatchMock.mockResolvedValue(false);
    useBillingAccessMock.mockReturnValue({
      isPaid: true,
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "cloud",
        base_url: "https://api.test/stt",
        api_key: "paid-token",
      }),
      expect.any(Object),
    );
    expect(sonnerToastMessageMock).toHaveBeenCalledWith(
      "Using a batch transcription provider",
      expect.objectContaining({
        description:
          "nova-3 is not available for batch transcription. Using Pro cloud transcription instead.",
      }),
    );
  });

  test("uses the configured final model for the batch pass (external provider)", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "nova-2-medical"
          : [],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "deepgram",
        model: "nova-2-medical",
        base_url: "https://api.deepgram.com/v1/listen",
        api_key: "test-key",
      }),
      expect.any(Object),
    );
    expect(startServerMock).not.toHaveBeenCalled();
    expect(stopServerMock).not.toHaveBeenCalled();
  });

  test("an explicit model option overrides the configured final model", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "nova-2-medical"
          : [],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", { model: "nova-3" });
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "deepgram",
        model: "nova-3",
      }),
      expect.any(Object),
    );
  });

  test("falls back to the live model when the final model matches it", async () => {
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language" ? "en" : key === "final_stt_model" ? "nova-3" : [],
    );
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "deepgram",
        model: "nova-3",
      }),
      expect.any(Object),
    );
    expect(startServerMock).not.toHaveBeenCalled();
  });

  test("swaps the local server to the final model and restores the live one", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "QuantizedSmall"
          : [],
    );
    startServerMock.mockResolvedValue({
      status: "ok",
      data: "http://127.0.0.1:6666",
    });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(isModelDownloadedMock).toHaveBeenCalledWith("QuantizedSmall");
    expect(startServerMock).toHaveBeenNthCalledWith(1, "QuantizedSmall");
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "QuantizedSmall",
        base_url: "http://127.0.0.1:6666",
      }),
      expect.any(Object),
    );
    // Restore happens after the batch run.
    expect(startServerMock).toHaveBeenNthCalledWith(2, "QuantizedTiny");
    expect(startTranscriptionMock.mock.invocationCallOrder[0]).toBeLessThan(
      startServerMock.mock.invocationCallOrder[1],
    );
  });

  test("keeps the live local model when the final model is not downloaded", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "QuantizedSmall"
          : [],
    );
    isModelDownloadedMock.mockResolvedValue({ status: "ok", data: false });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(startServerMock).not.toHaveBeenCalled();
    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "QuantizedTiny",
        base_url: "http://127.0.0.1:5555",
      }),
      expect.any(Object),
    );
  });

  test("an explicit local target overrides an external live connection", async () => {
    // The "Re-transcribe with…" picker passes model/baseUrl/provider for a
    // local model while the live connection may be an external provider.
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        model: "QuantizedSmall",
        baseUrl: "http://127.0.0.1:6666",
        provider: "hyprnote",
      });
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "QuantizedSmall",
        base_url: "http://127.0.0.1:6666",
      }),
      expect.any(Object),
    );
    // The caller owns the server swap for explicit targets.
    expect(startServerMock).not.toHaveBeenCalled();
    expect(sonnerToastMessageMock).not.toHaveBeenCalled();
  });

  test("an explicit target works without a live connection", async () => {
    useSTTConnectionMock.mockReturnValue({ conn: null });
    startTranscriptionMock.mockResolvedValue(undefined);

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav", {
        model: "QuantizedSmall",
        baseUrl: "http://127.0.0.1:6666",
        provider: "hyprnote",
      });
    });

    expect(startTranscriptionMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: "hyprnote",
        model: "QuantizedSmall",
        base_url: "http://127.0.0.1:6666",
        api_key: "",
      }),
      expect.any(Object),
    );
  });

  test("persists the batch transcript with replaceSession", async () => {
    startTranscriptionMock.mockImplementation(async (_params, options) => {
      options.handlePersist(
        [{ text: "hello", start_ms: 0, end_ms: 100, channel: 0 }],
        [],
      );
    });

    const { result } = renderHook(() => useRunBatch("session-1"));

    await act(async () => {
      await result.current("/tmp/session.wav");
    });

    expect(createTranscriptMock).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "session-1",
        source: "batch_transcription",
        replaceSession: true,
      }),
    );
  });

  test("restores the live local server when batch transcription fails", async () => {
    useSTTConnectionMock.mockReturnValue({
      conn: {
        provider: "hyprnote",
        model: "QuantizedTiny",
        baseUrl: "http://127.0.0.1:5555",
        apiKey: "",
      },
    });
    useConfigValueMock.mockImplementation((key) =>
      key === "ai_language"
        ? "en"
        : key === "final_stt_model"
          ? "QuantizedSmall"
          : [],
    );
    startTranscriptionMock.mockRejectedValue(new Error("provider failed"));

    const { result } = renderHook(() => useRunBatch("session-1"));

    await expect(
      act(async () => {
        await result.current("/tmp/session.wav");
      }),
    ).rejects.toThrow("provider failed");

    expect(startServerMock).toHaveBeenNthCalledWith(1, "QuantizedSmall");
    expect(startServerMock).toHaveBeenNthCalledWith(2, "QuantizedTiny");
  });
});

describe("getSessionSpeakerCount", () => {
  test("counts distinct session participants plus the current user", () => {
    expect(
      getSessionSpeakerCount(["human-a", "human-a", "human-b"], "self"),
    ).toBe(3);
  });

  test("returns undefined until at least two speakers are known", () => {
    expect(getSessionSpeakerCount(["human-a"], null)).toBe(undefined);
  });
});
