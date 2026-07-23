import { describe, expect, it } from "vitest";

import {
  type ActionItemLine,
  hasActionItemsSection,
  renderActionItemsSection,
  SECTION_END,
  SECTION_START,
  upsertActionItemsSection,
  writeActionItemsToMarkdown,
} from "./section";

const items: ActionItemLine[] = [
  { text: "Send the revised budget", done: false, dueAt: "2026-07-24", ownerLabel: "Alice" },
  { text: "Book the venue", done: true },
];

describe("renderActionItemsSection", () => {
  it("renders checkbox + due + owner, wrapped in markers", () => {
    const s = renderActionItemsSection(items);
    expect(s.startsWith(SECTION_START)).toBe(true);
    expect(s.endsWith(SECTION_END)).toBe(true);
    expect(s).toContain("## Action Items");
    expect(s).toContain("- [ ] Send the revised budget 📅 2026-07-24 @Alice");
    expect(s).toContain("- [x] Book the venue");
  });

  it("multi-word owners are hyphen-joined (@First-Last)", () => {
    const s = renderActionItemsSection([{ text: "t", done: false, ownerLabel: "Bob Smith" }]);
    expect(s).toContain("@Bob-Smith");
  });

  it("returns empty for no items", () => {
    expect(renderActionItemsSection([])).toBe("");
  });
});

describe("upsertActionItemsSection — replace-only-region (Obsidian-safe)", () => {
  const note = "# Meeting notes\n\nSome body the user wrote.\n\nMore notes below.\n";

  it("appends the section when absent, preserving the note", () => {
    const out = upsertActionItemsSection(note, items);
    expect(out).toContain("Some body the user wrote.");
    expect(out).toContain("More notes below.");
    expect(hasActionItemsSection(out)).toBe(true);
    // The user's content comes before the section.
    expect(out.indexOf("More notes below.")).toBeLessThan(out.indexOf(SECTION_START));
  });

  it("replaces ONLY the marked region on regeneration, preserving surroundings", () => {
    const withSection = upsertActionItemsSection(note, items);
    // Simulate the user adding text AFTER the section.
    const edited = `${withSection}\n\nUser added a footer after the section.\n`;

    const regenerated = upsertActionItemsSection(edited, [
      { text: "A brand new task", done: false },
    ]);

    // Old items gone, new item present.
    expect(regenerated).not.toContain("Send the revised budget");
    expect(regenerated).toContain("A brand new task");
    // Everything outside the markers is preserved.
    expect(regenerated).toContain("Some body the user wrote.");
    expect(regenerated).toContain("User added a footer after the section.");
    expect(hasActionItemsSection(regenerated)).toBe(true);
  });

  it("preserves content that sits BEFORE and AFTER an existing section", () => {
    const doc = `Intro paragraph.\n\n${renderActionItemsSection(items)}\n\nClosing paragraph.\n`;
    const out = upsertActionItemsSection(doc, [{ text: "only this", done: false }]);
    expect(out).toContain("Intro paragraph.");
    expect(out).toContain("Closing paragraph.");
    expect(out).toContain("only this");
    expect(out).not.toContain("Book the venue");
  });

  it("removes the section entirely when items become empty (SQLite has none)", () => {
    const withSection = `Body.\n\n${renderActionItemsSection(items)}\n\nAfter.\n`;
    const out = upsertActionItemsSection(withSection, []);
    expect(hasActionItemsSection(out)).toBe(false);
    expect(out).not.toContain("## Action Items");
    expect(out).toContain("Body.");
    expect(out).toContain("After.");
  });

  it("is idempotent: upserting the same items twice is stable", () => {
    const once = upsertActionItemsSection(note, items);
    const twice = upsertActionItemsSection(once, items);
    expect(twice).toBe(once);
  });

  it("no-op when there's no section and no items", () => {
    expect(upsertActionItemsSection(note, [])).toBe(note);
  });
});

describe("writeActionItemsToMarkdown (DB rows -> section)", () => {
  it("adapts rows + resolves owner labels + upserts", () => {
    const out = writeActionItemsToMarkdown(
      "Note body.\n",
      [
        { text: "ship it", status: "todo", due_at: "2026-08-01", owner_speaker_id: "spk_1" },
        { text: "done thing", status: "done", due_at: "", owner_speaker_id: null },
      ],
      (id) => (id === "spk_1" ? "Alice" : ""),
    );
    expect(out).toContain("- [ ] ship it 📅 2026-08-01 @Alice");
    expect(out).toContain("- [x] done thing");
    expect(out).toContain("Note body.");
  });

  it("empty rows removes the section", () => {
    const withSection = writeActionItemsToMarkdown("Body.\n", [
      { text: "x", status: "todo" },
    ]);
    expect(hasActionItemsSection(withSection)).toBe(true);
    const cleared = writeActionItemsToMarkdown(withSection, []);
    expect(hasActionItemsSection(cleared)).toBe(false);
  });
});
