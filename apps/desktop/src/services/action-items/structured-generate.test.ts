import { describe, expect, it, vi } from "vitest";
import { z } from "zod";

import { generateStructured } from "./structured-generate";

const schema = z.object({ items: z.array(z.object({ text: z.string() })) });

describe("generateStructured", () => {
  it("ollama: POSTs /api/chat with format + think:false and parses message.content", async () => {
    const fetchFn = vi.fn(async (url: string, init: RequestInit) => {
      expect(url).toBe("http://localhost:11434/api/chat");
      const body = JSON.parse(init.body as string);
      expect(body.model).toBe("qwen3:8b");
      expect(body.think).toBe(false);
      expect(body.stream).toBe(false);
      // The zod schema is forwarded as ollama's grammar-constraining `format`.
      expect(body.format).toBeTruthy();
      expect(body.format.type).toBe("object");
      return {
        ok: true,
        json: async () => ({
          message: { content: '{"items":[{"text":"do X"}]}' },
        }),
      } as unknown as Response;
    });

    const out = await generateStructured(
      {} as never,
      { schema, prompt: "p" },
      {
        providerId: "ollama",
        modelId: "qwen3:8b",
        baseUrl: "http://localhost:11434/v1",
      },
      { fetchFn: fetchFn as unknown as typeof fetch },
    );
    expect(out.items[0].text).toBe("do X");
    expect(fetchFn).toHaveBeenCalledOnce();
  });

  it("ollama: throws on non-JSON content (so the caller can fall back)", async () => {
    const fetchFn = async () =>
      ({
        ok: true,
        json: async () => ({ message: { content: "<think>hmm</think>" } }),
      }) as unknown as Response;
    await expect(
      generateStructured(
        {} as never,
        { schema, prompt: "p" },
        { providerId: "ollama", modelId: "m", baseUrl: "http://h/v1" },
        {
          fetchFn: fetchFn as unknown as typeof fetch,
        },
      ),
    ).rejects.toThrow(/non-JSON/);
  });

  it("ollama: throws on a non-2xx response", async () => {
    const fetchFn = async () =>
      ({
        ok: false,
        status: 500,
        text: async () => "boom",
      }) as unknown as Response;
    await expect(
      generateStructured(
        {} as never,
        { schema, prompt: "p" },
        { providerId: "ollama", modelId: "m", baseUrl: "http://h/v1" },
        {
          fetchFn: fetchFn as unknown as typeof fetch,
        },
      ),
    ).rejects.toThrow(/500/);
  });

  it("non-ollama: uses the injected AI-SDK generateObject and returns .object", async () => {
    const generateObjectFn = vi.fn(async () => ({
      object: { items: [{ text: "y" }] },
    }));
    const out = await generateStructured(
      {} as never,
      { schema, prompt: "p" },
      {
        providerId: "openai",
        modelId: "gpt-4o",
        baseUrl: "https://api.openai.com/v1",
      },
      { generateObjectFn: generateObjectFn as never },
    );
    expect(out.items[0].text).toBe("y");
    expect(generateObjectFn).toHaveBeenCalledOnce();
  });
});
