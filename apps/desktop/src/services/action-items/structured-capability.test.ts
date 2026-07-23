import { describe, expect, it, vi } from "vitest";

import { checkStructuredCapability } from "./structured-capability";

describe("checkStructuredCapability", () => {
  it("exempts ollama without probing (native format guarantees JSON)", async () => {
    const probe = vi.fn(async () => false);
    const r = await checkStructuredCapability(
      {
        providerId: "ollama",
        modelId: "qwen3:8b",
        baseUrl: "http://localhost:11434/v1",
      },
      probe,
    );
    expect(r.ok).toBe(true);
    expect(probe).not.toHaveBeenCalled();
  });

  it("passes a probe-capable non-ollama endpoint", async () => {
    const r = await checkStructuredCapability(
      {
        providerId: "openai",
        modelId: "gpt-4o",
        baseUrl: "https://api.openai.com/v1",
      },
      async () => true,
    );
    expect(r.ok).toBe(true);
  });

  it("fails a non-ollama endpoint whose probe fails (the PG gate)", async () => {
    const r = await checkStructuredCapability(
      { providerId: "custom", modelId: "mystery", baseUrl: "https://x/v1" },
      async () => false,
    );
    expect(r).toEqual({ ok: false, reason: "probe_failed" });
  });
});
