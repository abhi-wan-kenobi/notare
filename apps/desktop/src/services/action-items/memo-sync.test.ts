import { describe, expect, it, vi } from "vitest";

import {
  type ActionItemToggleRow,
  reconcileActionItemToggles,
  syncMemoActionItemToggles,
} from "./memo-sync";
import { parseActionItemsSection, writeActionItemsToMarkdown } from "./section";

const NOW = "2026-07-24T12:00:00.000Z";

// A memo whose Action Items section holds one item "Send budget", ticked done.
const SECTION_MARKERS_MEMO = writeActionItemsToMarkdown("Body.\n", [
  { text: "Send budget", status: "todo" },
]).replace("- [ ] Send budget", "- [x] Send budget");

describe("reconcileActionItemToggles (pure diff)", () => {
  const rows: ActionItemToggleRow[] = [
    { id: "ai-1", text: "Send budget", status: "todo" },
    { id: "ai-2", text: "Book venue", status: "done" },
    { id: "ai-3", text: "Draft agenda", status: "todo" },
  ];

  it("marks a newly-checked item done with completed_at set", () => {
    const updates = reconcileActionItemToggles(
      rows,
      [{ text: "Send budget", done: true }],
      NOW,
    );
    expect(updates).toEqual([{ id: "ai-1", status: "done", completedAt: NOW }]);
  });

  it("reverts a newly-unchecked item to todo with completed_at cleared", () => {
    const updates = reconcileActionItemToggles(
      rows,
      [{ text: "Book venue", done: false }],
      NOW,
    );
    expect(updates).toEqual([
      { id: "ai-2", status: "todo", completedAt: null },
    ]);
  });

  it("emits nothing when checkbox state already matches SQLite", () => {
    const updates = reconcileActionItemToggles(
      rows,
      [
        { text: "Send budget", done: false },
        { text: "Book venue", done: true },
      ],
      NOW,
    );
    expect(updates).toEqual([]);
  });

  it("ignores checkbox lines that match no row (text edits are not synced)", () => {
    const updates = reconcileActionItemToggles(
      rows,
      [{ text: "Send the budget NOW", done: true }],
      NOW,
    );
    expect(updates).toEqual([]);
  });

  it("treats 'completed' status as done", () => {
    const updates = reconcileActionItemToggles(
      [{ id: "x", text: "t", status: "completed" }],
      [{ text: "t", done: false }],
      NOW,
    );
    expect(updates).toEqual([{ id: "x", status: "todo", completedAt: null }]);
  });
});

describe("syncMemoActionItemToggles (applier)", () => {
  it("no-ops (no query, no write) when the memo has no section", async () => {
    const execute = vi.fn();
    const executeTransaction = vi.fn();
    const updates = await syncMemoActionItemToggles(
      "session-1",
      "# Notes\n\nnothing here",
      { execute, executeTransaction, now: () => NOW },
    );
    expect(updates).toEqual([]);
    expect(execute).not.toHaveBeenCalled();
    expect(executeTransaction).not.toHaveBeenCalled();
  });

  it("reads rows and writes the status update for a flipped checkbox", async () => {
    const execute = vi
      .fn()
      .mockResolvedValue([
        { id: "ai-1", text: "Send budget", status: "todo" },
      ] satisfies ActionItemToggleRow[]);
    const executeTransaction = vi.fn().mockResolvedValue(1);

    const memo = `${SECTION_MARKERS_MEMO}`;
    const updates = await syncMemoActionItemToggles("session-1", memo, {
      execute,
      executeTransaction,
      now: () => NOW,
    });

    expect(updates).toEqual([{ id: "ai-1", status: "done", completedAt: NOW }]);
    expect(execute).toHaveBeenCalledTimes(1);
    const [statements] = executeTransaction.mock.calls[0]!;
    expect(statements).toHaveLength(1);
    expect(statements[0].params).toEqual(["done", NOW, NOW, "ai-1"]);
    expect(statements[0].sql).toContain("UPDATE action_items");
  });

  it("does not write when nothing changed", async () => {
    const execute = vi
      .fn()
      .mockResolvedValue([
        { id: "ai-1", text: "Send budget", status: "done" },
      ] satisfies ActionItemToggleRow[]);
    const executeTransaction = vi.fn();

    const updates = await syncMemoActionItemToggles(
      "session-1",
      SECTION_MARKERS_MEMO,
      { execute, executeTransaction, now: () => NOW },
    );

    expect(updates).toEqual([]);
    expect(executeTransaction).not.toHaveBeenCalled();
  });
});

describe("full round-trip: render outbound -> flip on disk -> reconcile inbound", () => {
  it("a - [ ] flipped to - [x] on disk produces a done update for that row", () => {
    // Outbound: SQLite -> memo markdown. Owner is resolved via the supplied
    // resolver (fs-materializer passes owner_speaker_id through as the label).
    const memo = writeActionItemsToMarkdown(
      "# Meeting\n\nBody.\n",
      [
        {
          text: "Send budget",
          status: "todo",
          due_at: "2026-07-24",
          owner_speaker_id: "spk_1",
        },
        { text: "Book venue", status: "todo" },
      ],
      (id) => id,
    );
    expect(memo).toContain("- [ ] Send budget 📅 2026-07-24 @spk_1");

    // External edit on disk: user ticks the first box.
    const edited = memo.replace(
      "- [ ] Send budget 📅 2026-07-24 @spk_1",
      "- [x] Send budget 📅 2026-07-24 @spk_1",
    );

    // Inbound: parse the edited file, then reconcile against SQLite rows.
    const parsed = parseActionItemsSection(edited);
    expect(parsed).toEqual([
      { text: "Send budget", done: true },
      { text: "Book venue", done: false },
    ]);

    const rows: ActionItemToggleRow[] = [
      { id: "ai-1", text: "Send budget", status: "todo" },
      { id: "ai-2", text: "Book venue", status: "todo" },
    ];
    const updates = reconcileActionItemToggles(rows, parsed, NOW);
    expect(updates).toEqual([{ id: "ai-1", status: "done", completedAt: NOW }]);
  });
});
