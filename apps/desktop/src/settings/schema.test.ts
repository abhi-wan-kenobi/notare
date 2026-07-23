import { describe, expect, it } from "vitest";

import { SETTING_DEFINITIONS, type SettingKey } from "./schema";

describe("SETTING_DEFINITIONS meeting_bar_theme", () => {
  it("registers meeting_bar_theme with the conventions of dictation_orb_variant", () => {
    const definition = SETTING_DEFINITIONS.meeting_bar_theme;
    expect(definition).toBeDefined();
    expect(definition.type).toBe("string");
    expect(definition.path).toEqual(["general", "meeting_bar_theme"]);
    expect(definition.default).toBe("notare");
  });

  it("exposes meeting_bar_theme as a SettingKey", () => {
    const keys = Object.keys(SETTING_DEFINITIONS) as SettingKey[];
    expect(keys).toContain("meeting_bar_theme");
  });

  it("keeps the dictation_orb_variant definition unchanged", () => {
    expect(SETTING_DEFINITIONS.dictation_orb_variant).toEqual({
      type: "string",
      path: ["general", "dictation_orb_variant"],
      default: "cobalt",
    });
  });

  it("keeps the floating_bar_enabled definition unchanged", () => {
    expect(SETTING_DEFINITIONS.floating_bar_enabled).toEqual({
      type: "boolean",
      path: ["general", "floating_bar_enabled"],
      default: true,
    });
  });
});
