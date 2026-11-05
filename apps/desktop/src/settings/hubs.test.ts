import { describe, expect, it } from "vitest";

import { resolveSettingsHub } from "./hubs";

describe("resolveSettingsHub", () => {
  it.each([
    ["general", "general"],
    ["app", "general"],
    ["notifications", "general"],
    ["permissions", "general"],
    ["personalization", "general"],
    ["dictation", "general"],
    ["ai", "ai"],
    ["intelligence", "ai"],
    ["transcription", "ai"],
    ["recipes", "ai"],
    ["integrations", "integrations"],
    ["calendar", "integrations"],
    ["sync", "integrations"],
    ["gmail", "integrations"],
    ["mcp", "integrations"],
    ["webhook", "integrations"],
    ["account", "account"],
    ["storage", "account"],
    ["todo", "account"],
    ["export", "account"],
    ["delete", "account"],
  ] as const)("maps %s into hub %s", (tab, hub) => {
    expect(resolveSettingsHub(tab)).toBe(hub);
  });

  it("defaults missing settings state to general", () => {
    expect(resolveSettingsHub(null)).toBe("general");
    expect(resolveSettingsHub(undefined)).toBe("general");
  });
});
