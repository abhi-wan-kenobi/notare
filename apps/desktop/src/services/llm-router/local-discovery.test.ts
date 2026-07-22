import { describe, expect, it, vi } from "vitest";

import { listLmStudioModels, probeStructuredOutputs } from "./local-discovery";

/** Builds a minimal OpenAI-shaped chat-completions success response. */
function chatResponse(content: unknown) {
  return {
    ok: true,
    status: 200,
    json: async () => ({
      choices: [{ message: { content } }],
    }),
  } as unknown as Response;
}

function okResponse(json: unknown) {
  return {
    ok: true,
    status: 200,
    json: async () => json,
  } as unknown as Response;
}

function httpErrorResponse() {
  return {
    ok: false,
    status: 500,
    json: async () => ({ error: "boom" }),
  } as unknown as Response;
}

describe("probeStructuredOutputs", () => {
  it("returns true when the model echoes valid JSON matching the schema", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(chatResponse('{"ok": true}'));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "qwen3:8b",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(true);

    // The request targets the OpenAI-compatible chat-completions path and
    // asks for a json_schema response_format.
    const [url, init] = fetchImpl.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://localhost:1234/v1/chat/completions");
    expect(init.method).toBe("POST");
    const body = JSON.parse(init.body as string) as {
      response_format: { type: string };
    };
    expect(body.response_format.type).toBe("json_schema");
  });

  it("appends /v1 when the base URL has no version segment (ollama)", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(chatResponse('{"ok": false}'));
    await probeStructuredOutputs(
      "http://localhost:11434",
      "llama3.2",
      fetchImpl as unknown as typeof fetch,
    );
    const [url] = fetchImpl.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://localhost:11434/v1/chat/completions");
  });

  it("returns true for valid JSON with surrounding whitespace", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(chatResponse('   \n {"ok": true} \n   '));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(true);
  });

  it("returns false when the content is malformed JSON", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(chatResponse("not json"));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false when the parsed JSON does not match the schema", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(chatResponse('{"ok": "yes"}'));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false when the response shape is missing choices/content", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(okResponse({ choices: [] }));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false on HTTP error", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(httpErrorResponse());
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false on a network/throwing fetch", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("ECONNREFUSED"));
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false when the response body is not valid JSON", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => {
        throw new SyntaxError("Unexpected token");
      },
    } as unknown as Response);
    const result = await probeStructuredOutputs(
      "http://localhost:1234/v1",
      "m",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toBe(false);
  });

  it("returns false for empty base URL or model id", async () => {
    const fetchImpl = vi.fn();
    expect(
      await probeStructuredOutputs(
        "",
        "m",
        fetchImpl as unknown as typeof fetch,
      ),
    ).toBe(false);
    expect(
      await probeStructuredOutputs(
        "http://localhost:1234/v1",
        "",
        fetchImpl as unknown as typeof fetch,
      ),
    ).toBe(false);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe("listLmStudioModels", () => {
  it("returns model ids from the OpenAI-compatible /v1/models payload", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        okResponse({ data: [{ id: "qwen3:8b" }, { id: "llama3.2" }] }),
      );
    const result = await listLmStudioModels(
      "http://localhost:1234/v1",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toEqual(["qwen3:8b", "llama3.2"]);
    expect(fetchImpl.mock.calls[0][0]).toBe("http://localhost:1234/v1/models");
  });

  it("returns [] on HTTP error", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(httpErrorResponse());
    const result = await listLmStudioModels(
      "http://localhost:1234/v1",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toEqual([]);
  });

  it("returns [] when the fetch throws", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("ECONNREFUSED"));
    const result = await listLmStudioModels(
      "http://localhost:1234/v1",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toEqual([]);
  });

  it("returns [] when the payload shape is unexpected", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(okResponse({ models: [] }));
    const result = await listLmStudioModels(
      "http://localhost:1234/v1",
      fetchImpl as unknown as typeof fetch,
    );
    expect(result).toEqual([]);
  });

  it("returns [] for an empty base URL", async () => {
    const fetchImpl = vi.fn();
    expect(
      await listLmStudioModels("", fetchImpl as unknown as typeof fetch),
    ).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});
