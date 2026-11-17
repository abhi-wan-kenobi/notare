import { describe, expect, it } from "vitest";

import { SETTING_DEFINITIONS, type SettingKey } from "./schema";

describe("SETTING_DEFINITIONS floating control", () => {
  it("does not expose obsolete theme or shape settings", () => {
    expect(SETTING_DEFINITIONS).not.toHaveProperty("meeting_bar_theme");
    expect(SETTING_DEFINITIONS).not.toHaveProperty("dictation_orb_variant");
  });

  it("keeps the floating_bar_enabled definition unchanged", () => {
    expect(SETTING_DEFINITIONS.floating_bar_enabled).toEqual({
      type: "boolean",
      path: ["general", "floating_bar_enabled"],
      default: true,
    });
  });
});

describe("SETTING_DEFINITIONS sync_enabled", () => {
  it("defaults to off — compiling the sync feature in must not be enough on its own", () => {
    expect(SETTING_DEFINITIONS.sync_enabled).toEqual({
      type: "boolean",
      path: ["sync", "enabled"],
      default: false,
    });
  });

  it("exposes sync_enabled as a SettingKey", () => {
    const keys = Object.keys(SETTING_DEFINITIONS) as SettingKey[];
    expect(keys).toContain("sync_enabled");
  });
});
