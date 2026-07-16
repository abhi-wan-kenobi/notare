import { describe, expect, it, vi } from "vitest";

import {
  type FinalizeDictationDeps,
  type FinalizeDictationInput,
  finalizeDictation,
  normalizeCleanupMode,
} from "./finalize";

function makeDeps(
  overrides: Partial<FinalizeDictationDeps> = {},
): FinalizeDictationDeps {
  return {
    cleanBasic: vi.fn(async (text: string) => `basic(${text})`),
    cleanLlm: vi.fn(async (text: string) => `llm(${text})`),
    deliver: vi.fn(async () => undefined),
    saveHistory: vi.fn(async () => undefined),
    onLlmFallback: vi.fn(),
    ...overrides,
  };
}

function makeInput(
  overrides: Partial<FinalizeDictationInput> = {},
): FinalizeDictationInput {
  return {
    rawText: "hello world",
    mode: "batch",
    failed: false,
    cleanup: "basic",
    pasteAtCursor: true,
    ...overrides,
  };
}

describe("normalizeCleanupMode", () => {
  it("defaults everything unknown to basic", () => {
    expect(normalizeCleanupMode("none")).toBe("none");
    expect(normalizeCleanupMode("llm")).toBe("llm");
    expect(normalizeCleanupMode("basic")).toBe("basic");
    expect(normalizeCleanupMode(undefined)).toBe("basic");
    expect(normalizeCleanupMode("garbage")).toBe("basic");
  });
});

describe("finalizeDictation cleanup dispatch", () => {
  it("none keeps the raw text and marks the entry raw", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ cleanup: "none" }), deps);

    expect(deps.cleanBasic).not.toHaveBeenCalled();
    expect(deps.cleanLlm).not.toHaveBeenCalled();
    expect(deps.deliver).toHaveBeenCalledWith("hello world", true);
    expect(deps.saveHistory).toHaveBeenCalledWith({
      text: "hello world",
      mode: "batch",
      cleaned: false,
    });
  });

  it("basic runs the deterministic cleaner", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ cleanup: "basic" }), deps);

    expect(deps.cleanBasic).toHaveBeenCalledWith("hello world");
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith({
      text: "basic(hello world)",
      mode: "batch",
      cleaned: true,
    });
  });

  it("llm uses the model cleaner when available", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(deps.cleanLlm).toHaveBeenCalledWith("hello world");
    expect(deps.cleanBasic).not.toHaveBeenCalled();
    expect(deps.onLlmFallback).not.toHaveBeenCalled();
    expect(deps.deliver).toHaveBeenCalledWith("llm(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith({
      text: "llm(hello world)",
      mode: "batch",
      cleaned: true,
    });
  });

  it("llm falls back to basic when no model is configured", async () => {
    const deps = makeDeps({ cleanLlm: null });
    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(deps.onLlmFallback).toHaveBeenCalledWith(null);
    expect(deps.cleanBasic).toHaveBeenCalledWith("hello world");
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
  });

  it("llm falls back to basic when the model call fails", async () => {
    const error = new Error("boom");
    const deps = makeDeps({ cleanLlm: vi.fn(async () => Promise.reject(error)) });
    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(deps.onLlmFallback).toHaveBeenCalledWith(error);
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
  });

  it("llm falls back to basic on an empty model answer", async () => {
    const deps = makeDeps({ cleanLlm: vi.fn(async () => "   ") });
    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(deps.onLlmFallback).toHaveBeenCalledTimes(1);
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
  });
});

describe("finalizeDictation delivery matrix", () => {
  it("batch + paste-at-cursor pastes", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ pasteAtCursor: true }), deps);
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
  });

  it("batch + copy-only never pastes", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ pasteAtCursor: false }), deps);
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", false);
  });

  it("a failed batch session degrades to copy-only", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({ failed: true, pasteAtCursor: true }),
      deps,
    );
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", false);
  });

  it("type mode never delivers but still records history", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ mode: "type" }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).toHaveBeenCalledWith({
      text: "basic(hello world)",
      mode: "type",
      cleaned: true,
    });
  });

  it("still saves history when delivery fails", async () => {
    const deps = makeDeps({
      deliver: vi.fn(async () => Promise.reject(new Error("no clipboard"))),
    });
    const errorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    try {
      await finalizeDictation(makeInput(), deps);
    } finally {
      errorSpy.mockRestore();
    }

    expect(deps.saveHistory).toHaveBeenCalledTimes(1);
  });
});

describe("finalizeDictation empty transcripts", () => {
  it("does nothing for empty or whitespace-only raw text", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ rawText: "   " }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).not.toHaveBeenCalled();
  });

  it("skips delivery and history when cleanup strips everything", async () => {
    const deps = makeDeps({ cleanBasic: vi.fn(async () => "") });
    await finalizeDictation(makeInput({ rawText: "[BLANK_AUDIO]" }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).not.toHaveBeenCalled();
  });
});
