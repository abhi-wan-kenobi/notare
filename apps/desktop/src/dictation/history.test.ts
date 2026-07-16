import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    async (_statements: { sql: string; params: unknown[] }[]) => undefined,
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: vi.fn(() => ({ data: undefined })),
}));

import {
  addDictationHistoryEntry,
  clearDictationHistory,
  deleteDictationHistoryEntry,
  DICTATION_HISTORY_CAP,
} from "./history";

type Statement = { sql: string; params: unknown[] };

function statements(call = 0): Statement[] {
  return mocks.executeTransaction.mock.calls[call][0];
}

describe("dictation history writes", () => {
  beforeEach(() => {
    mocks.executeTransaction.mockClear();
  });

  it("inserts the entry and prunes past the cap in one transaction", async () => {
    await addDictationHistoryEntry({
      text: "Hello world",
      mode: "batch",
      cleaned: true,
    });

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const [insert, prune] = statements();

    expect(insert.sql).toContain("INSERT INTO dictation_history");
    const [id, text, mode, cleaned] = insert.params;
    expect(typeof id).toBe("string");
    expect(text).toBe("Hello world");
    expect(mode).toBe("batch");
    expect(cleaned).toBe(1);
    // ISO timestamp.
    expect(String(insert.params[4])).toMatch(/^\d{4}-\d{2}-\d{2}T/);

    expect(prune.sql).toContain("DELETE FROM dictation_history");
    expect(prune.sql).toContain("NOT IN");
    expect(prune.params).toEqual([DICTATION_HISTORY_CAP]);
  });

  it("stores raw entries with cleaned = 0", async () => {
    await addDictationHistoryEntry({
      text: "raw",
      mode: "type",
      cleaned: false,
    });

    const [insert] = statements();
    expect(insert.params[2]).toBe("type");
    expect(insert.params[3]).toBe(0);
  });

  it("deletes a single entry by id", async () => {
    await deleteDictationHistoryEntry("some-id");

    const [statement] = statements();
    expect(statement.sql).toContain("DELETE FROM dictation_history WHERE id = ?");
    expect(statement.params).toEqual(["some-id"]);
  });

  it("clears the whole history", async () => {
    await clearDictationHistory();

    const [statement] = statements();
    expect(statement.sql.trim()).toBe("DELETE FROM dictation_history");
  });
});
