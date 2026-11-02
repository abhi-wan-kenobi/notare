import { describe, expect, it, vi } from "vitest";

import { fetchOmpModels, parseOmpModels } from "./list-omp";

function okResponse(json: unknown) {
  return {
    ok: true,
    status: 200,
    json: async () => json,
  } as unknown as Response;
}

describe("parseOmpModels", () => {
  it("parses omp aliases and upstream provider/model ids", () => {
    const raw = [
      { id: "@smol" },
      { id: "@slow" },
      { id: "@plan" },
      { id: "anthropic/claude-3-5-sonnet" },
      { id: "openai/gpt-4o" },
    ];

    const result = parseOmpModels(raw);

    expect(result.models).toEqual([
      "@smol",
      "@slow",
      "@plan",
      "anthropic/claude-3-5-sonnet",
      "openai/gpt-4o",
    ]);
    expect(result.ignored).toEqual([]);
    expect(result.metadata["@smol"]).toEqual({ input_modalities: ["text"] });
    expect(result.metadata["anthropic/claude-3-5-sonnet"]).toEqual({
      input_modalities: ["text"],
    });
  });

  it("handles string items as well as object items", () => {
    const result = parseOmpModels(["@fast", { id: "@smart" }]);
    expect(result.models).toEqual(["@fast", "@smart"]);
  });

  it("filters out invalid, whitespace, or missing ids", () => {
    const raw = [
      { id: "@smol" },
      { id: "" },
      { id: "   " },
      { id: 123 },
      { id: null },
      {},
      "@valid",
    ];

    const result = parseOmpModels(raw as unknown as Array<{ id?: unknown }>);
    expect(result.models).toEqual(["@smol", "@valid"]);
  });

  it("returns empty result for empty input", () => {
    const result = parseOmpModels([]);
    expect(result).toEqual({
      models: [],
      ignored: [],
      metadata: {},
    });
  });
});

describe("fetchOmpModels", () => {
  it("fetches from the gateway and parses models on 200", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      okResponse({
        data: [{ id: "@smol" }, { id: "@slow" }],
      }),
    );

    const result = await fetchOmpModels(
      "http://127.0.0.1:4000/v1",
      "secret-token",
      fetchImpl as unknown as typeof fetch,
    );

    expect(result.models).toEqual(["@smol", "@slow"]);
    expect(fetchImpl).toHaveBeenCalledWith(
      "http://127.0.0.1:4000/v1/models",
      expect.objectContaining({
        headers: { Authorization: "Bearer secret-token" },
      }),
    );
  });

  it("returns empty list on 401 without throwing", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      json: async () => ({ error: "unauthorized" }),
    } as unknown as Response);

    const result = await fetchOmpModels(
      "http://127.0.0.1:4000/v1",
      undefined,
      fetchImpl as unknown as typeof fetch,
    );

    expect(result.models).toEqual([]);
  });

  it("returns empty list on ECONNREFUSED without throwing", async () => {
    const fetchImpl = vi
      .fn()
      .mockRejectedValue(new Error("connect ECONNREFUSED 127.0.0.1:4000"));

    const result = await fetchOmpModels(
      "http://127.0.0.1:4000/v1",
      undefined,
      fetchImpl as unknown as typeof fetch,
    );

    expect(result.models).toEqual([]);
  });

  it("returns empty list for empty baseUrl without invoking fetch", async () => {
    const fetchImpl = vi.fn();
    const result = await fetchOmpModels(
      "",
      undefined,
      fetchImpl as unknown as typeof fetch,
    );
    expect(result.models).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});
