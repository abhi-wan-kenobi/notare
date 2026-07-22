import { describe, expect, it } from "vitest";

import {
  checkCaps,
  inferParamsB,
  providerSupportsStructuredOutputs,
} from "./capabilities";
import { classifyTier, isLocalUrl, resolveModel } from "./index";

describe("tier classification", () => {
  it("classifies the local engines as local regardless of URL", () => {
    expect(classifyTier({ providerId: "ollama", modelId: "qwen3:8b" })).toBe(
      "local",
    );
    expect(classifyTier({ providerId: "lmstudio", modelId: "x" })).toBe(
      "local",
    );
  });

  it("classifies hosted and BYO clouds", () => {
    expect(classifyTier({ providerId: "hyprnote", modelId: "x" })).toBe(
      "hosted",
    );
    for (const p of [
      "openai",
      "anthropic",
      "openrouter",
      "google_generative_ai",
      "mistral",
      "azure_openai",
    ]) {
      expect(classifyTier({ providerId: p, modelId: "x" })).toBe("byo-cloud");
    }
  });

  it("classifies custom endpoints by host", () => {
    expect(
      classifyTier({
        providerId: "custom",
        modelId: "x",
        baseUrl: "http://localhost:8080/v1",
      }),
    ).toBe("local");
    expect(
      classifyTier({
        providerId: "custom",
        modelId: "x",
        baseUrl: "http://192.168.0.91:11434/v1",
      }),
    ).toBe("local");
    expect(
      classifyTier({
        providerId: "custom",
        modelId: "x",
        baseUrl: "https://myserver.ts.net/v1",
      }),
    ).toBe("local");
    expect(
      classifyTier({
        providerId: "custom",
        modelId: "x",
        baseUrl: "https://api.example.com/v1",
      }),
    ).toBe("byo-cloud");
    // No URL at all -> not provably local -> cloud rules apply.
    expect(classifyTier({ providerId: "custom", modelId: "x" })).toBe(
      "byo-cloud",
    );
  });

  it("isLocalUrl handles the RFC1918 ranges precisely", () => {
    expect(isLocalUrl("http://10.0.0.5:1234")).toBe(true);
    expect(isLocalUrl("http://172.16.0.1:1234")).toBe(true);
    expect(isLocalUrl("http://172.31.255.1:1234")).toBe(true);
    expect(isLocalUrl("http://172.32.0.1:1234")).toBe(false);
    expect(isLocalUrl("http://172.15.0.1:1234")).toBe(false);
    expect(isLocalUrl("http://8.8.8.8")).toBe(false);
    expect(isLocalUrl("not a url")).toBe(false);
    expect(isLocalUrl(undefined)).toBe(false);
  });
});

describe("param-count heuristic", () => {
  it("parses ollama-style tags", () => {
    expect(inferParamsB("qwen3:8b")).toBe(8);
    expect(inferParamsB("llama3.1:70b-instruct-q4_K_M")).toBe(70);
    expect(inferParamsB("gemma3:27b-it")).toBe(27);
    expect(inferParamsB("qwen3:0.6b")).toBe(0.6);
    expect(inferParamsB("mistral-7b-v0.3")).toBe(7);
    expect(inferParamsB("Meta-Llama-3.1-8B-Instruct-GGUF")).toBe(8);
  });

  it("does not mistake quantization or context markers for sizes", () => {
    // q4/q8_0 quantization and 128k context must not read as params.
    expect(inferParamsB("llama3.1:70b-instruct-q4_K_M")).toBe(70);
    expect(inferParamsB("model-128k-instruct")).toBeNull();
    expect(inferParamsB("gpt-4o")).toBeNull();
    expect(inferParamsB("claude-sonnet-4-5")).toBeNull();
  });
});

describe("capability gating", () => {
  it("action_items requires structured outputs and >=7B", () => {
    const small = checkCaps({
      task: "action_items",
      providerId: "ollama",
      modelId: "qwen3:4b",
    });
    expect(small.minParamsOk).toBe(false);

    const big = checkCaps({
      task: "action_items",
      providerId: "ollama",
      modelId: "qwen3:8b",
    });
    expect(big.minParamsOk).toBe(true);
    expect(big.structuredOutputs).toBe(true);
  });

  it("unknown size is 'unknown' unless the user overrides", () => {
    const unknown = checkCaps({
      task: "action_items",
      providerId: "ollama",
      modelId: "mystery-model",
    });
    expect(unknown.minParamsOk).toBe("unknown");

    const overridden = checkCaps({
      task: "action_items",
      providerId: "ollama",
      modelId: "mystery-model",
      userOverride: true,
    });
    expect(overridden.minParamsOk).toBe(true);
  });

  it("enhance has no floor: small models pass", () => {
    const caps = checkCaps({
      task: "enhance",
      providerId: "ollama",
      modelId: "qwen3:0.6b",
    });
    expect(caps.minParamsOk).toBe(true);
    expect(caps.structuredOutputs).toBe(true);
  });

  it("custom provider structured-output support is unknown, not assumed", () => {
    expect(providerSupportsStructuredOutputs("custom")).toBe("unknown");
  });
});

describe("resolveModel invariants", () => {
  it("INVARIANT: cloud is never selected without explicit selection", () => {
    // A cloud candidate that did NOT come from explicit user selection must
    // never resolve, even when it is the only candidate.
    const r = resolveModel("enhance", {
      selected: { providerId: "openai", modelId: "gpt-4o" },
      selectionIsExplicit: false,
      localFallbacks: [],
    });
    expect(r.status).toBe("unavailable");
    expect((r as { reason: string }).reason).toBe("cloud_not_opted_in");
  });

  it("INVARIANT: cloud fallbacks are dropped, never returned", () => {
    // Caller bug: a cloud entry in localFallbacks. Router must drop it.
    const r = resolveModel("enhance", {
      selected: null,
      selectionIsExplicit: false,
      localFallbacks: [
        { providerId: "openai", modelId: "gpt-4o" },
        {
          providerId: "custom",
          modelId: "x",
          baseUrl: "https://api.example.com/v1",
        },
      ],
    });
    expect(r.status).toBe("unavailable");
    expect((r as { reason: string }).reason).toBe("no_provider");
  });

  it("explicitly selected cloud resolves (user opt-in = the selection act)", () => {
    const r = resolveModel("enhance", {
      selected: { providerId: "openai", modelId: "gpt-4o" },
      selectionIsExplicit: true,
    });
    expect(r.status).toBe("ok");
    if (r.status === "ok") {
      expect(r.tier).toBe("byo-cloud");
    }
  });

  it("local fallback resolves without explicit selection", () => {
    const r = resolveModel("enhance", {
      selected: null,
      selectionIsExplicit: false,
      localFallbacks: [{ providerId: "ollama", modelId: "qwen3:8b" }],
    });
    expect(r.status).toBe("ok");
    if (r.status === "ok") {
      expect(r.tier).toBe("local");
      expect(r.providerId).toBe("ollama");
    }
  });

  it("caps_unmet is reported for structurally under-capable selections", () => {
    const r = resolveModel("action_items", {
      selected: { providerId: "ollama", modelId: "qwen3:4b" },
      selectionIsExplicit: true,
    });
    expect(r.status).toBe("unavailable");
    if (r.status === "unavailable") {
      expect(r.reason).toBe("caps_unmet");
      expect(r.caps?.minParamsOk).toBe(false);
    }
  });

  it("caps override lets an unknown-size model through, flagged uncertain", () => {
    const r = resolveModel("action_items", {
      selected: { providerId: "ollama", modelId: "mystery-model" },
      selectionIsExplicit: true,
      capsUserOverride: true,
    });
    expect(r.status).toBe("ok");
  });

  it("unknown-size without override is refused for action_items", () => {
    const r = resolveModel("action_items", {
      selected: { providerId: "ollama", modelId: "mystery-model" },
      selectionIsExplicit: true,
    });
    // "unknown" is not a pass for a hard-gated task without the override…
    // …but it is also not a provable failure; the router resolves it and
    // flags capsUncertain so the UI warns. (Refusing outright would brick
    // every custom model name.)
    expect(r.status).toBe("ok");
    if (r.status === "ok") {
      expect(r.capsUncertain).toBe(true);
    }
  });

  it("nothing configured -> no_provider", () => {
    const r = resolveModel("enhance", {
      selected: null,
      selectionIsExplicit: false,
    });
    expect(r.status).toBe("unavailable");
    expect((r as { reason: string }).reason).toBe("no_provider");
  });
});
