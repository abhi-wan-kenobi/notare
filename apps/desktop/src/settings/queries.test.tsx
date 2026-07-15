import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  getPreferredLanguages: vi.fn(),
  setProperties: vi.fn(async () => undefined),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  listSupportedModels: vi.fn(async () => ({
    status: "ok" as const,
    data: [] as Array<{ key: string }>,
  })),
  isModelDownloaded: vi.fn(async (_model: string) => ({
    status: "ok" as const,
    data: false,
  })),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    setDisabled: vi.fn(async () => undefined),
    setProperties: mocks.setProperties,
  },
}));

vi.mock("@hypr/plugin-detect", () => ({
  commands: {
    getPreferredLanguages: mocks.getPreferredLanguages,
  },
}));

vi.mock("@hypr/plugin-local-stt", () => ({
  commands: {
    listSupportedModels: mocks.listSupportedModels,
    isModelDownloaded: mocks.isModelDownloaded,
    startServer: vi.fn(async () => ({ status: "ok", data: null })),
    stopServer: vi.fn(async () => ({ status: "ok", data: null })),
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: vi.fn(() => ({ data: undefined })),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  adoptSttModelIfUnconfigured,
  initializeApplicationSettings,
  parseSettingRows,
  setSettingValues,
  updateSettingValue,
} from "./queries";

describe("SQLite settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.execute.mockResolvedValue([]);
    mocks.getPreferredLanguages.mockResolvedValue({
      status: "error",
      error: "unavailable",
    });
    mocks.listSupportedModels.mockResolvedValue({ status: "ok", data: [] });
    mocks.isModelDownloaded.mockResolvedValue({ status: "ok", data: false });
  });

  it("maps the imported settings document into typed values", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: {
            theme: "dark",
            save_recordings: false,
          },
          language: {
            spoken_languages: ["en", "ko"],
          },
          notification: {
            ignored_platforms: ["com.example.video"],
          },
        }),
      },
    ]);

    expect(result.values.theme).toBe("dark");
    expect(result.values.audio_retention).toBe("none");
    expect(result.values.spoken_languages).toBe('["en","ko"]');
    expect(result.values.ignored_platforms).toBe('["com.example.video"]');
    expect(result.hasValues.has("theme")).toBe(true);
  });

  it("prefers valid direct rows and falls back from corrupt ones", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: { theme: "dark", week_start: "monday" },
        }),
      },
      { id: "theme", value_json: JSON.stringify("light") },
      { id: "week_start", value_json: "not-json" },
    ]);

    expect(result.values.theme).toBe("light");
    expect(result.values.week_start).toBe("monday");
  });

  it("recovers main-store values after settings document values and aliases", () => {
    const result = parseSettingRows([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          general: { ai_language: "fr" },
          language: { spoken_languages: ["fr"] },
        }),
      },
      {
        id: "legacy_main_values_document",
        value_json: JSON.stringify({
          ai_language: "ko",
          spoken_languages: JSON.stringify(["ko"]),
          theme: "dark",
        }),
      },
    ]);

    expect(result.values.ai_language).toBe("fr");
    expect(result.values.spoken_languages).toBe('["fr"]');
    expect(result.values.theme).toBe("dark");
  });

  it("writes multiple independent values in one transaction", async () => {
    await setSettingValues({
      theme: "dark",
      notification_event: false,
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("INSERT INTO app_settings");
    expect(statements[0].sql).toContain("ON CONFLICT(id) DO UPDATE");
    expect(statements[0].params.slice(0, 2)).toEqual([
      "theme",
      JSON.stringify("dark"),
    ]);
    expect(statements[1].params.slice(0, 2)).toEqual([
      "notification_event",
      JSON.stringify(false),
    ]);
  });

  it("persists OS language defaults only when no stored values exist", async () => {
    let rows: Array<{ id: string; value_json: string }> = [];
    mocks.execute.mockImplementation(async () => rows);
    mocks.executeTransaction.mockImplementation(async (statements) => {
      rows = statements.map((statement) => ({
        id: String(statement.params[0]),
        value_json: String(statement.params[1]),
      }));
      return statements.map(() => 1);
    });
    mocks.getPreferredLanguages.mockResolvedValue({
      status: "ok",
      data: ["ko", "en"],
    });

    await initializeApplicationSettings();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements.map((statement) => statement.params.slice(0, 2))).toEqual(
      [
        ["ai_language", JSON.stringify("ko")],
        ["spoken_languages", JSON.stringify(JSON.stringify(["ko", "en"]))],
        ["current_stt_provider", JSON.stringify("hyprnote")],
      ],
    );
  });

  it("repairs a selected external transcription provider with no model", async () => {
    let rows = [
      {
        id: "current_stt_provider",
        value_json: JSON.stringify("deepgram"),
      },
      { id: "current_stt_model", value_json: JSON.stringify("") },
    ];
    mocks.execute.mockImplementation(async () => rows);
    mocks.executeTransaction.mockImplementation(async (statements) => {
      rows = statements.map((statement) => ({
        id: String(statement.params[0]),
        value_json: String(statement.params[1]),
      }));
      return statements.map(() => 1);
    });

    await initializeApplicationSettings();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements.map((statement) => statement.params.slice(0, 2))).toEqual(
      [["current_stt_model", JSON.stringify("nova-3-general")]],
    );
  });

  it("adopts an already-downloaded local model when none is selected on startup", async () => {
    let rows = [
      { id: "current_stt_provider", value_json: JSON.stringify("hyprnote") },
    ];
    mocks.execute.mockImplementation(async () => rows);
    mocks.executeTransaction.mockImplementation(async (statements) => {
      rows = [
        ...rows.filter(
          (row) =>
            !statements.some((statement) => statement.params[0] === row.id),
        ),
        ...statements.map((statement) => ({
          id: String(statement.params[0]),
          value_json: String(statement.params[1]),
        })),
      ];
      return statements.map(() => 1);
    });
    mocks.listSupportedModels.mockResolvedValue({
      status: "ok",
      data: [{ key: "QuantizedTiny" }, { key: "QuantizedSmall" }],
    });
    mocks.isModelDownloaded.mockImplementation(async (model) => ({
      status: "ok",
      data: model === "QuantizedSmall",
    }));

    await initializeApplicationSettings();

    expect(rows).toContainEqual({
      id: "current_stt_model",
      value_json: JSON.stringify("QuantizedSmall"),
    });
    expect(rows).toContainEqual({
      id: "current_stt_provider",
      value_json: JSON.stringify("hyprnote"),
    });
  });

  it("selects a freshly downloaded model when nothing valid is configured", async () => {
    mocks.execute.mockResolvedValue([
      { id: "current_stt_provider", value_json: JSON.stringify("hyprnote") },
    ]);

    await expect(adoptSttModelIfUnconfigured("QuantizedSmall")).resolves.toBe(
      true,
    );

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements.map((statement) => statement.params.slice(0, 2))).toEqual(
      [
        ["current_stt_provider", JSON.stringify("hyprnote")],
        ["current_stt_model", JSON.stringify("QuantizedSmall")],
      ],
    );
  });

  it("does not override an existing model selection on download completion", async () => {
    mocks.execute.mockResolvedValue([
      { id: "current_stt_provider", value_json: JSON.stringify("hyprnote") },
      { id: "current_stt_model", value_json: JSON.stringify("QuantizedTiny") },
    ]);

    await expect(adoptSttModelIfUnconfigured("QuantizedSmall")).resolves.toBe(
      false,
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("updates against the latest SQLite value inside the write queue", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "personalization_dictionary_terms",
        value_json: JSON.stringify(JSON.stringify(["Notare"])),
      },
    ]);

    const next = await updateSettingValue(
      "personalization_dictionary_terms",
      (current) => JSON.stringify([...JSON.parse(current ?? "[]"), "Erebor"]),
    );

    expect(next).toBe(JSON.stringify(["Notare", "Erebor"]));
    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.params.slice(0, 2)).toEqual([
      "personalization_dictionary_terms",
      JSON.stringify(next),
    ]);
  });
});
