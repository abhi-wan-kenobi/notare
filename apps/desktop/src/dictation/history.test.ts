import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    async (_statements: { sql: string; params: unknown[] }[]) => undefined,
  ),
  execute: vi.fn(async (_sql: string, _params: unknown[]) => [] as unknown[]),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  useLiveQuery: vi.fn(() => ({ data: undefined })),
  liveQueryClient: { execute: mocks.execute },
}));

import {
  addDictationHistoryEntry,
  clearDictationHistory,
  deleteDictationHistoryEntry,
  DICTATION_HISTORY_PRUNE_CAP,
  listDictationHistory,
  pruneDictationHistoryByAge,
  setDictationHistoryPinned,
  updateDictationHistoryText,
} from "./history";

type Statement = { sql: string; params: unknown[] };

function statements(call = 0): Statement[] {
  return mocks.executeTransaction.mock.calls[call][0];
}

function makeRow(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id: "row-id",
    text: "cleaned text",
    raw_text: "raw text",
    source: "dictation",
    model: "QuantizedTiny",
    duration_ms: 1200,
    pinned: 0,
    status: "delivered",
    created_at: "2026-07-31T00:00:00.000Z",
    ...overrides,
  };
}

describe("dictation history writes", () => {
  beforeEach(() => {
    mocks.executeTransaction.mockClear();
    mocks.execute.mockClear();
    mocks.execute.mockResolvedValue([]);
  });

  it("inserts the full entry and prunes unpinned past the cap in one transaction", async () => {
    await addDictationHistoryEntry({
      text: "Hello world",
      rawText: "um hello world",
      mode: "batch",
      cleaned: true,
      source: "dictation",
      model: "QuantizedTiny",
      durationMs: 4200,
      status: "delivered",
    });

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
    const [insert, prune] = statements();

    expect(insert.sql).toContain("INSERT INTO dictation_history");
    const [
      id,
      text,
      rawText,
      mode,
      cleaned,
      source,
      model,
      durationMs,
      status,
    ] = insert.params;
    expect(typeof id).toBe("string");
    expect(text).toBe("Hello world");
    expect(rawText).toBe("um hello world");
    expect(mode).toBe("batch");
    expect(cleaned).toBe(1);
    expect(source).toBe("dictation");
    expect(model).toBe("QuantizedTiny");
    expect(durationMs).toBe(4200);
    expect(status).toBe("delivered");
    // ISO timestamp.
    expect(String(insert.params[9])).toMatch(/^\d{4}-\d{2}-\d{2}T/);

    // Pruning only ever considers unpinned rows, so pinned entries are exempt.
    expect(prune.sql).toContain("DELETE FROM dictation_history");
    expect(prune.sql).toContain("pinned = 0");
    expect(prune.params).toEqual([DICTATION_HISTORY_PRUNE_CAP]);
  });

  it("keeps the cap at a rolling 500 unpinned rows", () => {
    expect(DICTATION_HISTORY_PRUNE_CAP).toBe(500);
  });

  it("stays backward compatible with the old { text, mode, cleaned } shape", async () => {
    await addDictationHistoryEntry({
      text: "raw",
      mode: "type",
      cleaned: false,
    });

    const [insert] = statements();
    const [, text, rawText, mode, cleaned, source, model, durationMs, status] =
      insert.params;
    expect(text).toBe("raw");
    expect(rawText).toBeNull();
    expect(mode).toBe("type");
    expect(cleaned).toBe(0);
    // Defaults fill in for the omitted new fields.
    expect(source).toBe("dictation");
    expect(model).toBeNull();
    expect(durationMs).toBeNull();
    expect(status).toBe("delivered");
  });

  it("stores a discarded recovery entry with its status", async () => {
    await addDictationHistoryEntry({
      text: "",
      rawText: "salvageable mumble",
      mode: "batch",
      cleaned: true,
      status: "discarded",
    });

    const [insert] = statements();
    expect(insert.params[2]).toBe("salvageable mumble");
    expect(insert.params[8]).toBe("discarded");
  });

  it("sets and clears the pinned flag", async () => {
    await setDictationHistoryPinned("some-id", true);
    let [statement] = statements();
    expect(statement.sql).toContain("UPDATE dictation_history SET pinned = ?");
    expect(statement.params).toEqual([1, "some-id"]);

    mocks.executeTransaction.mockClear();
    await setDictationHistoryPinned("some-id", false);
    [statement] = statements();
    expect(statement.params).toEqual([0, "some-id"]);
  });

  it("deletes a single entry by id", async () => {
    await deleteDictationHistoryEntry("some-id");

    const [statement] = statements();
    expect(statement.sql).toContain(
      "DELETE FROM dictation_history WHERE id = ?",
    );
    expect(statement.params).toEqual(["some-id"]);
  });

  it("clears the whole history", async () => {
    await clearDictationHistory();

    const [statement] = statements();
    expect(statement.sql.trim()).toBe("DELETE FROM dictation_history");
  });

  // Snippets inline edit (v0.5.1 Lane A1): the FTS mirror updates via the
  // existing `dictation_history_fts_au` AFTER UPDATE trigger, not a second
  // write here.
  it("updates an entry's text by id", async () => {
    await updateDictationHistoryText("some-id", "corrected text");

    const [statement] = statements();
    expect(statement.sql).toContain("UPDATE dictation_history SET text = ?");
    expect(statement.sql).toContain("WHERE id = ?");
    expect(statement.params).toEqual(["corrected text", "some-id"]);
  });

  // v0.5.1 history-retention lane: `retention` is the enforcement hook for
  // write-time age-based pruning (additive to the existing count-based cap).
  describe("age-based retention on insert", () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-07-31T00:00:00.000Z"));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("does not add a third statement when retention is omitted", async () => {
      await addDictationHistoryEntry({
        text: "no retention passed",
        mode: "batch",
        cleaned: true,
      });

      expect(statements()).toHaveLength(2);
    });

    it('does not add a third statement when retention is "off"', async () => {
      await addDictationHistoryEntry({
        text: "off",
        mode: "batch",
        cleaned: true,
        retention: "off",
      });

      expect(statements()).toHaveLength(2);
    });

    it("adds a third unpinned-only age-prune statement for a real retention window", async () => {
      await addDictationHistoryEntry({
        text: "with retention",
        mode: "batch",
        cleaned: true,
        retention: "30d",
      });

      const [, , agePrune] = statements();
      expect(agePrune.sql).toContain("DELETE FROM dictation_history");
      expect(agePrune.sql).toContain("pinned = 0");
      expect(agePrune.sql).toContain("created_at < ?");
      expect(agePrune.params).toEqual(["2026-07-01T00:00:00.000Z"]);
    });
  });
});

describe("pruneDictationHistoryByAge", () => {
  beforeEach(() => {
    mocks.executeTransaction.mockClear();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-31T00:00:00.000Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('is a no-op for "off"', async () => {
    await pruneDictationHistoryByAge("off");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("is a no-op for an unrecognized retention value", async () => {
    await pruneDictationHistoryByAge("not-a-real-tier");
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it.each([
    ["7d", "2026-07-24T00:00:00.000Z"],
    ["30d", "2026-07-01T00:00:00.000Z"],
    ["90d", "2026-05-02T00:00:00.000Z"],
  ])(
    "computes the %s cutoff and deletes unpinned rows past it",
    async (retention, expectedCutoff) => {
      await pruneDictationHistoryByAge(retention);

      expect(mocks.executeTransaction).toHaveBeenCalledTimes(1);
      const [statement] = statements();
      // Pinned rows are exempt regardless of status (delivered or discarded) -
      // there is no status filter here.
      expect(statement.sql).toContain("DELETE FROM dictation_history");
      expect(statement.sql).toContain("pinned = 0");
      expect(statement.sql).toContain("created_at < ?");
      expect(statement.sql).not.toContain("status");
      expect(statement.params).toEqual([expectedCutoff]);
    },
  );
});

describe("listDictationHistory", () => {
  beforeEach(() => {
    mocks.execute.mockClear();
    mocks.execute.mockResolvedValue([]);
  });

  it("returns a first page newest-first with no FTS join and maps rows", async () => {
    mocks.execute.mockResolvedValue([
      makeRow({ id: "a", pinned: 1, status: "delivered" }),
    ]);

    const { entries, nextCursor } = await listDictationHistory();

    const [sql, params] = mocks.execute.mock.calls[0];
    expect(sql).not.toContain("MATCH");
    expect(sql).not.toContain("JOIN dictation_history_fts");
    expect(sql).toContain("ORDER BY h.created_at DESC, h.id DESC");
    // Default limit 50, fetch one extra to detect a further page.
    expect(params).toEqual([51]);

    expect(entries).toHaveLength(1);
    expect(entries[0]).toEqual({
      id: "a",
      text: "cleaned text",
      rawText: "raw text",
      source: "dictation",
      model: "QuantizedTiny",
      durationMs: 1200,
      pinned: true,
      status: "delivered",
      createdAt: "2026-07-31T00:00:00.000Z",
    });
    expect(nextCursor).toBeNull();
  });

  it("full-text searches over text AND raw_text with a sanitized query", async () => {
    mocks.execute.mockResolvedValue([]);

    await listDictationHistory({ query: 'quarterly "rev' });

    const [sql, params] = mocks.execute.mock.calls[0];
    // Joined on the stable TEXT id, never the implicit rowid (VACUUM can
    // renumber implicit rowids and would desync a rowid-coupled FTS join).
    expect(sql).toContain("JOIN dictation_history_fts f ON f.id = h.id");
    expect(sql).toContain("dictation_history_fts MATCH ?");
    // Each token is quoted + prefixed; embedded quotes are doubled, so raw FTS
    // operators can never throw a syntax error.
    expect(params[0]).toBe('"quarterly"* """rev"*');
    expect(params[params.length - 1]).toBe(51);
  });

  it("paginates by keyset cursor without OFFSET", async () => {
    // First page: two rows returned for a limit of 1 -> hasMore.
    mocks.execute.mockResolvedValueOnce([
      makeRow({ id: "newer", created_at: "2026-07-31T00:00:02.000Z" }),
      makeRow({ id: "older", created_at: "2026-07-31T00:00:01.000Z" }),
    ]);
    const first = await listDictationHistory({ limit: 1 });
    expect(first.entries).toHaveLength(1);
    expect(first.entries[0].id).toBe("newer");
    expect(first.nextCursor).not.toBeNull();

    // Second page: feed the cursor back; keyset predicate must be present.
    mocks.execute.mockResolvedValueOnce([]);
    await listDictationHistory({ limit: 1, cursor: first.nextCursor });

    const [sql, params] = mocks.execute.mock.calls[1];
    expect(sql).not.toContain("OFFSET");
    expect(sql).toContain("h.created_at < ?");
    expect(params).toEqual([
      "2026-07-31T00:00:02.000Z",
      "2026-07-31T00:00:02.000Z",
      "newer",
      2,
    ]);
  });

  it("restarts from the first page on a malformed cursor", async () => {
    mocks.execute.mockResolvedValue([]);
    await listDictationHistory({ cursor: "not-a-cursor" });

    const [sql, params] = mocks.execute.mock.calls[0];
    expect(sql).not.toContain("h.created_at < ?");
    expect(params).toEqual([51]);
  });

  // btoa/atob alone are Latin-1-only; the cursor goes through UTF-8 bytes so
  // an id carrying non-Latin-1 text round-trips instead of throwing.
  it("round-trips a cursor whose id contains non-Latin-1 text", async () => {
    mocks.execute.mockResolvedValueOnce([
      makeRow({ id: "टिप्पणी-1", created_at: "2026-07-31T00:00:02.000Z" }),
      makeRow({ id: "older", created_at: "2026-07-31T00:00:01.000Z" }),
    ]);
    const first = await listDictationHistory({ limit: 1 });
    expect(first.nextCursor).not.toBeNull();

    mocks.execute.mockResolvedValueOnce([]);
    await listDictationHistory({ limit: 1, cursor: first.nextCursor });

    const [, params] = mocks.execute.mock.calls[1];
    expect(params).toEqual([
      "2026-07-31T00:00:02.000Z",
      "2026-07-31T00:00:02.000Z",
      "टिप्पणी-1",
      2,
    ]);
  });
});
