import { describe, expect, it, vi } from "vitest";

import type { DictionaryEntry } from "./dictionary";
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

/** Full saveHistory shape with the defaults finalize always threads. */
function saved(
  partial: Partial<Parameters<FinalizeDictationDeps["saveHistory"]>[0]>,
) {
  return {
    text: "basic(hello world)",
    rawText: "hello world",
    mode: "batch" as const,
    cleaned: true,
    source: "dictation" as const,
    model: null,
    durationMs: null,
    status: "delivered" as const,
    ...partial,
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
    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ text: "hello world", cleaned: false }),
    );
  });

  it("basic runs the deterministic cleaner", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ cleanup: "basic" }), deps);

    expect(deps.cleanBasic).toHaveBeenCalledWith("hello world");
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith(saved({}));
  });

  it("llm uses the model cleaner when available", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(deps.cleanLlm).toHaveBeenCalledWith("hello world");
    expect(deps.cleanBasic).not.toHaveBeenCalled();
    expect(deps.onLlmFallback).not.toHaveBeenCalled();
    expect(deps.deliver).toHaveBeenCalledWith("llm(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ text: "llm(hello world)" }),
    );
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

describe("finalizeDictation raw threading + provenance", () => {
  it("stores the pre-cleanup raw transcript alongside the cleaned text", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({ rawText: "  um hello   world  " }),
      deps,
    );

    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({
        text: "basic(um hello   world)",
        rawText: "um hello   world",
      }),
    );
  });

  it("threads the model name and duration when the host provides them", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({ model: "QuantizedTiny", durationMs: 4200 }),
      deps,
    );

    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ model: "QuantizedTiny", durationMs: 4200 }),
    );
  });

  it("always tags dictation-path saves with source 'dictation'", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ mode: "type" }), deps);

    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ mode: "type", source: "dictation" }),
    );
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

  it("type mode never delivers but still records history", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ mode: "type" }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).toHaveBeenCalledWith(saved({ mode: "type" }));
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

describe("finalizeDictation discarded-dictation recovery", () => {
  it("saves the raw transcript as discarded when a session failed", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({ failed: true, pasteAtCursor: true }),
      deps,
    );

    // A failed session degrades to copy-only, and the entry is flagged
    // discarded so it surfaces in recovery rather than the clipboard list.
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", false);
    expect(deps.saveHistory).toHaveBeenCalledWith(saved({ status: "discarded" }));
  });

  it("keeps the raw transcript when cleanup strips everything to nothing", async () => {
    const deps = makeDeps({ cleanBasic: vi.fn(async () => "") });
    await finalizeDictation(makeInput({ rawText: "[BLANK_AUDIO]" }), deps);

    // Nothing to deliver, but the raw transcript is preserved for recovery.
    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ text: "", rawText: "[BLANK_AUDIO]", status: "discarded" }),
    );
  });

  it("never persists empty or whitespace-only raw text", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ rawText: "   " }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).not.toHaveBeenCalled();
  });
});

describe("finalizeDictation phase signaling", () => {
  it("holds processing across cleanup and delivery for a clean batch", async () => {
    const order: string[] = [];
    const deps = makeDeps({
      cleanLlm: vi.fn(async (text: string) => {
        order.push("clean");
        return `llm(${text})`;
      }),
      deliver: vi.fn(async () => {
        order.push("deliver");
      }),
      saveHistory: vi.fn(async () => {
        order.push("history");
      }),
      signalPhase: vi.fn((phase: "processing" | "idle") => {
        order.push(`phase:${phase}`);
      }),
    });

    await finalizeDictation(makeInput({ cleanup: "llm" }), deps);

    expect(order).toEqual([
      "phase:processing",
      "clean",
      "deliver",
      "history",
      "phase:idle",
    ]);
  });

  it("returns the orb to idle even when delivery throws", async () => {
    const signalPhase = vi.fn();
    const errorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    try {
      await finalizeDictation(
        makeInput(),
        makeDeps({
          deliver: vi.fn(async () => Promise.reject(new Error("no paste"))),
          signalPhase,
        }),
      );
    } finally {
      errorSpy.mockRestore();
    }

    expect(signalPhase).toHaveBeenNthCalledWith(1, "processing");
    expect(signalPhase).toHaveBeenLastCalledWith("idle");
  });

  it("returns the orb to idle when cleanup strips everything", async () => {
    const signalPhase = vi.fn();
    await finalizeDictation(
      makeInput({ rawText: "[BLANK_AUDIO]" }),
      makeDeps({ cleanBasic: vi.fn(async () => ""), signalPhase }),
    );

    expect(signalPhase).toHaveBeenNthCalledWith(1, "processing");
    expect(signalPhase).toHaveBeenLastCalledWith("idle");
  });

  it("never signals for type mode or failed sessions", async () => {
    const signalPhase = vi.fn();
    await finalizeDictation(
      makeInput({ mode: "type" }),
      makeDeps({ signalPhase }),
    );
    await finalizeDictation(
      makeInput({ failed: true }),
      makeDeps({ signalPhase }),
    );

    expect(signalPhase).not.toHaveBeenCalled();
  });

  it("never signals for empty raw text", async () => {
    const signalPhase = vi.fn();
    await finalizeDictation(
      makeInput({ rawText: "   " }),
      makeDeps({ signalPhase }),
    );

    expect(signalPhase).not.toHaveBeenCalled();
  });
});

describe("finalizeDictation custom dictionary", () => {
  const mapping = (
    wrong: string,
    right: string,
    caseSensitive = false,
  ): DictionaryEntry => ({ wrong, right, caseSensitive });

  it("applies the dictionary after basic cleanup, before delivery + history", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({ dictionary: [mapping("world", "WORLD")] }),
      deps,
    );

    // cleanBasic wraps to "basic(hello world)"; the dictionary then rewrites
    // "world" -> "WORLD" on the already-cleaned text.
    expect(deps.cleanBasic).toHaveBeenCalledWith("hello world");
    expect(deps.deliver).toHaveBeenCalledWith("basic(hello WORLD)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({ text: "basic(hello WORLD)" }),
    );
  });

  it("leaves the raw transcript untouched by the dictionary", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({
        rawText: "far eye rocks",
        cleanup: "none",
        dictionary: [mapping("far eye", "FarEye")],
      }),
      deps,
    );

    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({
        // "none" cleanup keeps the transcript verbatim, then the dictionary
        // rewrites it; because it changed, the entry is marked cleaned.
        text: "FarEye rocks",
        rawText: "far eye rocks",
        cleaned: true,
      }),
    );
  });

  it("is a no-op with an empty dictionary", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ dictionary: [] }), deps);

    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith(saved({}));
  });

  it("is a no-op when the dictionary is omitted entirely", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput(), deps);

    expect(deps.deliver).toHaveBeenCalledWith("basic(hello world)", true);
    expect(deps.saveHistory).toHaveBeenCalledWith(saved({}));
  });

  it("applies to the type-mode history entry (no delivery)", async () => {
    const deps = makeDeps();
    await finalizeDictation(
      makeInput({
        mode: "type",
        cleanup: "none",
        rawText: "far eye",
        dictionary: [mapping("far eye", "FarEye")],
      }),
      deps,
    );

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).toHaveBeenCalledWith(
      saved({
        mode: "type",
        text: "FarEye",
        rawText: "far eye",
        cleaned: true,
      }),
    );
  });
});

describe("finalizeDictation empty transcripts", () => {
  it("does nothing for empty or whitespace-only raw text", async () => {
    const deps = makeDeps();
    await finalizeDictation(makeInput({ rawText: "   " }), deps);

    expect(deps.deliver).not.toHaveBeenCalled();
    expect(deps.saveHistory).not.toHaveBeenCalled();
  });
});
