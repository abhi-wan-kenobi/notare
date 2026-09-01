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

  // notare-local (the embedded llama.cpp server) deliberately gets NO
  // exemption here, unlike ollama: ollama's exemption exists because its
  // *native* format-enforced endpoint differs from the openai-compat
  // surface being probed, so a probe of the wrong surface false-negatives.
  // notare-local's /v1/chat/completions IS the endpoint that both the probe
  // and real extraction use — there's no separate native path to diverge
  // from — and it grammar-constrains response_format: json_schema via
  // llama.cpp's llguidance sampler, so the probe is expected to pass
  // honestly rather than needing a convenience carve-out.
  it("probes notare-local honestly (grammar constraint, not a convenience exemption)", async () => {
    const probe = vi.fn(async () => true);
    const r = await checkStructuredCapability(
      {
        providerId: "notare-local",
        modelId: "HyprLLM",
        baseUrl: "http://127.0.0.1:54213/v1",
      },
      probe,
    );
    expect(probe).toHaveBeenCalledWith("http://127.0.0.1:54213/v1", "HyprLLM");
    expect(r.ok).toBe(true);
  });

  it("fails notare-local if its probe ever fails, same as any other provider", async () => {
    const r = await checkStructuredCapability(
      {
        providerId: "notare-local",
        modelId: "HyprLLM",
        baseUrl: "http://127.0.0.1:54213/v1",
      },
      async () => false,
    );
    expect(r).toEqual({ ok: false, reason: "probe_failed" });
  });
});
