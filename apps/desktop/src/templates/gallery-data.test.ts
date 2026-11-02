import { describe, expect, it } from "vitest";

import { parseWebTemplates } from "./codec";
import { BUILT_IN_TEMPLATES } from "./gallery-data";

describe("BUILT_IN_TEMPLATES", () => {
  it("contains 25 built-in templates with categories and sections", () => {
    expect(BUILT_IN_TEMPLATES).toHaveLength(25);

    const parsed = parseWebTemplates(
      BUILT_IN_TEMPLATES as unknown as Record<string, unknown>[],
    );
    expect(parsed).toHaveLength(25);

    const titles = BUILT_IN_TEMPLATES.map((t) => t.title.toLowerCase());
    const expectedSubstrings = [
      "1:1",
      "sales discovery",
      "sales demo",
      "customer success check-in",
      "interview (hiring)",
      "user research interview",
      "standup",
      "sprint planning",
      "sprint retro",
      "design review",
      "architecture review",
      "incident postmortem",
      "board meeting",
      "investor update",
      "all-hands",
      "okr review",
      "performance review",
      "project kickoff",
      "project status",
      "vendor evaluation",
      "legal / contract review",
      "therapy / coaching session",
      "lecture / class",
      "podcast / interview recording",
      "personal journal",
    ];

    for (const expected of expectedSubstrings) {
      const match = titles.some((t) => t.includes(expected));
      expect(match, `Expected template matching "${expected}"`).toBe(true);
    }

    for (const template of BUILT_IN_TEMPLATES) {
      expect(template.title.trim().length).toBeGreaterThan(0);
      expect(template.description.trim().length).toBeGreaterThan(0);
      expect(template.category.trim().length).toBeGreaterThan(0);
      expect(template.sections.length).toBeGreaterThan(0);
      for (const section of template.sections) {
        expect(section.title.trim().length).toBeGreaterThan(0);
        expect(section.description.trim().length).toBeGreaterThan(0);
      }
    }
  });
});
