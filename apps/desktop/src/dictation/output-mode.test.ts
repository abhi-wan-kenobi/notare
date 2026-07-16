import { describe, expect, it } from "vitest";

import { isLegacyOutputMode, normalizeOutputMode } from "./output-mode";

describe("normalizeOutputMode", () => {
  it("passes the current values through", () => {
    expect(normalizeOutputMode("type")).toBe("type");
    expect(normalizeOutputMode("batch")).toBe("batch");
  });

  it("migrates the legacy batch-paste value to batch", () => {
    expect(normalizeOutputMode("batch-paste")).toBe("batch");
  });

  it("falls back to type for unknown or missing values", () => {
    expect(normalizeOutputMode(undefined)).toBe("type");
    expect(normalizeOutputMode("")).toBe("type");
    expect(normalizeOutputMode("garbage")).toBe("type");
  });
});

describe("isLegacyOutputMode", () => {
  it("flags only the pre-rework spelling", () => {
    expect(isLegacyOutputMode("batch-paste")).toBe(true);
    expect(isLegacyOutputMode("batch")).toBe(false);
    expect(isLegacyOutputMode("type")).toBe(false);
    expect(isLegacyOutputMode(undefined)).toBe(false);
  });
});
