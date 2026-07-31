import { describe, expect, it } from "vitest";

import {
  isCloudProvider,
  resolveScopeSelection,
  type ScopeSelectionInput,
} from "./scope";

function input(overrides: Partial<ScopeSelectionInput> = {}): ScopeSelectionInput {
  return {
    hasOverride: true,
    overrideKnown: true,
    overrideAvailable: true,
    overrideIsCloud: false,
    globalIsCloud: false,
    ...overrides,
  };
}

describe("isCloudProvider", () => {
  it("treats the local engines as non-cloud regardless of URL", () => {
    expect(isCloudProvider("ollama", "https://anything")).toBe(false);
    expect(isCloudProvider("lmstudio", undefined)).toBe(false);
  });

  it("classifies custom endpoints by host", () => {
    expect(isCloudProvider("custom", "http://localhost:11434/v1")).toBe(false);
    expect(isCloudProvider("custom", "http://192.168.0.91:11434/v1")).toBe(
      false,
    );
    expect(isCloudProvider("custom", "https://api.example.com/v1")).toBe(true);
    // No URL = not provably local -> cloud.
    expect(isCloudProvider("custom", undefined)).toBe(true);
  });

  it("treats every other provider (incl. hosted) as cloud", () => {
    for (const p of ["openai", "anthropic", "openrouter", "hyprnote"]) {
      expect(isCloudProvider(p, undefined)).toBe(true);
    }
    expect(isCloudProvider(undefined, undefined)).toBe(false);
  });
});

describe("resolveScopeSelection", () => {
  it("inherits the global selection when no override is set", () => {
    const r = resolveScopeSelection(input({ hasOverride: false }));
    expect(r.source).toBe("inherit");
    expect(r.fallbackReason).toBe("no_override");
  });

  it("uses a local override that is known + available", () => {
    const r = resolveScopeSelection(input());
    expect(r.source).toBe("override");
    expect(r.fallbackReason).toBeUndefined();
  });

  it("falls back when the override names an unknown provider", () => {
    const r = resolveScopeSelection(input({ overrideKnown: false }));
    expect(r.source).toBe("inherit");
    expect(r.fallbackReason).toBe("unknown_provider");
  });

  it("falls back when the override can't form a connection", () => {
    const r = resolveScopeSelection(input({ overrideAvailable: false }));
    expect(r.source).toBe("inherit");
    expect(r.fallbackReason).toBe("unavailable");
  });

  describe("cloud-opt-in invariant (the cleanup scope)", () => {
    it("INVARIANT: a cloud override is REFUSED when cloud is not opted in globally", () => {
      // The dictation-cleanup scope must never become a back-door to a cloud
      // endpoint the user never explicitly selected globally.
      const r = resolveScopeSelection(
        input({ overrideIsCloud: true, globalIsCloud: false }),
      );
      expect(r.source).toBe("inherit");
      expect(r.fallbackReason).toBe("cloud_not_opted_in");
    });

    it("allows a cloud override once cloud IS opted in globally", () => {
      const r = resolveScopeSelection(
        input({ overrideIsCloud: true, globalIsCloud: true }),
      );
      expect(r.source).toBe("override");
    });

    it("reports the cloud gate even when the override is also unavailable", () => {
      // The security-relevant reason wins over an incidental availability miss.
      const r = resolveScopeSelection(
        input({
          overrideIsCloud: true,
          globalIsCloud: false,
          overrideAvailable: false,
        }),
      );
      expect(r.fallbackReason).toBe("cloud_not_opted_in");
    });

    it("a local override is always allowed, cloud opt-in irrelevant", () => {
      const r = resolveScopeSelection(
        input({ overrideIsCloud: false, globalIsCloud: false }),
      );
      expect(r.source).toBe("override");
    });
  });
});
