import { cleanup, renderHook } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { DatabaseSync, SQLInputValue } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createMigratedDb, queryAll } from "~/calendar/test-sqlite";

const state = vi.hoisted(() => ({ db: null as unknown as DatabaseSync }));

vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: async (sql: string, params: unknown[] = []) => {
      const statement = state.db.prepare(sql);
      return statement.columns().length
        ? queryAll(state.db, sql, params)
        : (statement.run(...(params as SQLInputValue[])), []);
    },
  },
  executeTransaction: async (
    statements: Array<{ sql: string; params: unknown[] }>,
  ) => {
    state.db.exec("BEGIN");
    try {
      const changes = statements.map(({ sql, params }) =>
        Number(
          state.db.prepare(sql).run(...(params as SQLInputValue[])).changes,
        ),
      );
      state.db.exec("COMMIT");
      return changes;
    } catch (error) {
      state.db.exec("ROLLBACK");
      throw error;
    }
  },
  useLiveQuery: (options: { sql: string; params: unknown[] }) => ({
    data: queryAll(state.db, options.sql, options.params),
  }),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: vi.fn(async () => ({ status: "ok" })) },
}));
vi.mock("@hypr/plugin-fs-sync", () => ({ commands: {} }));

import { json2md, md2json } from "@hypr/editor/markdown";
import { extractTasksFromContent } from "@hypr/editor/tasks";

import { seriesIdForEvent } from "./identity";
import {
  extractTopics,
  mapOccurrenceRow,
  parseDecisionsFromNote,
  prepareSeriesOccurrence,
  SERIES_SQL,
  type SeriesOccurrenceSqlRow,
  useSeriesCarryOver,
} from "./index";

import { setActionItemStatus } from "~/session/components/action-items/queries";
import { getOrCreateSessionForEventId } from "~/session/queries";

const identity = {
  provider: "google",
  calendar_id: "work",
  recurrence_series_id: "provider-series",
};
const seriesId = seriesIdForEvent(identity);

function event(id: string, calendarId: string, date: string) {
  state.db
    .prepare(`INSERT INTO events (
    id, calendar_id, provider, tracking_id_event, recurrence_series_id,
    title, started_at, ended_at
  ) VALUES (?, ?, 'google', ?, 'provider-series', 'Planning', ?, ?)`)
    .run(id, calendarId, `external-${date}`, date, date);
}

function body(sessionId: string): string {
  return queryAll<{ body: string }>(
    state.db,
    "SELECT body FROM session_documents WHERE session_id = ? AND kind = 'note'",
    [sessionId],
  )[0].body;
}

async function seedPrevious() {
  event("work-first", "work", "2026-11-01T09:00:00Z");
  event("work-next", "work", "2026-11-08T09:00:00Z");
  const previousId = await getOrCreateSessionForEventId("work-first");
  state.db
    .prepare(`INSERT INTO action_items (
    id, session_id, source_type, source_id, text, status
  ) VALUES ('canonical-task', ?, 'session', ?, 'Ship release', 'todo')`)
    .run(previousId, previousId);
  return previousId;
}

describe("recurring meeting persistence", () => {
  beforeEach(() => {
    state.db = createMigratedDb();
  });
  afterEach(() => {
    cleanup();
    state.db.close();
  });

  it("isolates histories and event notes when calendars reuse provider ids", async () => {
    event("work-first", "work", "2026-11-01T09:00:00Z");
    event("home-first", "home", "2026-11-01T09:00:00Z");
    const work = await getOrCreateSessionForEventId("work-first");
    const home = await getOrCreateSessionForEventId("home-first");
    expect(work).not.toBe(home);
    expect(
      queryAll<{ id: string }>(state.db, SERIES_SQL, [seriesId]).map(
        (r) => r.id,
      ),
    ).toEqual([work]);
    event("home-next", "home", "2026-11-07T09:00:00Z");
    event("work-next", "work", "2026-11-08T09:00:00Z");
    const prepared = await prepareSeriesOccurrence(seriesId);
    expect(
      queryAll(state.db, "SELECT event_id FROM sessions WHERE id = ?", [
        prepared,
      ]),
    ).toEqual([{ event_id: "work-next" }]);
    await expect(
      prepareSeriesOccurrence(seriesId, "home-first"),
    ).rejects.toThrow(/series/i);
    await expect(prepareSeriesOccurrence(seriesId, "missing")).rejects.toThrow(
      /event/i,
    );
  });

  it("reuses prepared event notes without changing typed raw content, including concurrent prepares", async () => {
    await seedPrevious();
    const ids = await Promise.all([
      prepareSeriesOccurrence(seriesId, "work-next"),
      prepareSeriesOccurrence(seriesId, "work-next"),
    ]);
    expect(ids[0]).toBe(ids[1]);
    state.db
      .prepare("UPDATE session_documents SET body = ? WHERE session_id = ?")
      .run("user typed content", ids[0]);
    await expect(prepareSeriesOccurrence(seriesId, "work-next")).resolves.toBe(
      ids[0],
    );
    expect(body(ids[0])).toBe("user typed content");
    expect(
      queryAll(
        state.db,
        "SELECT id FROM sessions WHERE event_id = 'work-next'",
      ),
    ).toHaveLength(1);
  });

  it("references one canonical action instead of copying tasks or generating raw checkboxes", async () => {
    await seedPrevious();
    const carryOver = renderHook(() => useSeriesCarryOver(seriesId));
    expect(carryOver.result.current.data).toMatchObject([
      { id: "canonical-task" },
    ]);
    const next = await prepareSeriesOccurrence(seriesId, "work-next");
    const note = JSON.parse(body(next));
    expect(json2md(note)).toContain("Ship release");
    const taskNodes: unknown[] = [];
    type NoteNode = { type?: string; content?: NoteNode[] };
    const visit = (node: NoteNode) => {
      if (node.type === "taskItem") taskNodes.push(node);
      node.content?.forEach(visit);
    };
    visit(note);
    expect(
      extractTasksFromContent(note, { type: "session_raw_note", id: next }),
    ).toEqual([]);
    expect(taskNodes).toEqual([]);
    expect(queryAll(state.db, "SELECT id FROM action_items")).toEqual([
      { id: "canonical-task" },
    ]);
    carryOver.rerender();
    expect(carryOver.result.current.data).toMatchObject([
      { id: "canonical-task" },
    ]);
    await setActionItemStatus("canonical-task", "done", "user");
    carryOver.rerender();
    expect(carryOver.result.current.data).toEqual([]);
    event("work-third", "work", "2026-11-15T09:00:00Z");
    const third = await prepareSeriesOccurrence(seriesId, "work-third");
    expect(json2md(JSON.parse(body(third)))).not.toContain("Ship release");
    expect(queryAll(state.db, "SELECT id, status FROM action_items")).toEqual([
      { id: "canonical-task", status: "done" },
    ]);
  });

  it("reads the generated summary for TL;DR, topics, decisions and the next note's recap", async () => {
    const previousId = await seedPrevious();
    // The row services/enhancer/storage.ts writes once a summary is generated.
    state.db
      .prepare(
        `INSERT INTO session_documents (
          id, session_id, kind, title, body_format, body, sort_order
        ) VALUES ('summary-1', ?, 'summary', 'Summary', 'prosemirror_json', ?, 1)`,
      )
      .run(
        previousId,
        JSON.stringify(
          md2json(
            '# Launch plan\n\n- Ship on Tuesday\n\n# Risks\n\n- Vendor delay\n\n```decisions\n[{"text":"Ship on Tuesday","owner":"Alice"}]\n```',
          ),
        ),
      );

    const [occurrence] = queryAll<SeriesOccurrenceSqlRow>(
      state.db,
      SERIES_SQL,
      [seriesId],
    ).map(mapOccurrenceRow);

    expect(occurrence.tldr).toBe("Launch plan");
    expect(extractTopics(occurrence.enhanced_note)).toEqual([
      "Launch plan",
      "Risks",
    ]);
    expect(parseDecisionsFromNote(occurrence.enhanced_note)).toEqual([
      { text: "Ship on Tuesday", owner: "Alice" },
    ]);

    const next = await prepareSeriesOccurrence(seriesId, "work-next");
    expect(json2md(JSON.parse(body(next)))).toContain(
      "Last meeting: Launch plan",
    );
  });

  it("backfills linked/embedded events but preserves unlinked manual history and completed tasks", () => {
    event("work-first", "work", "2026-11-01T09:00:00Z");
    event("home-first", "home", "2026-11-01T09:00:00Z");
    state.db.exec(`INSERT INTO sessions (id, event_id, series_id) VALUES
      ('work-history', 'work-first', 'provider-series'),
      ('home-history', 'home-first', 'provider-series'),
      ('manual', '', 'manual-group');
      INSERT INTO action_items (id, session_id, status, text)
      VALUES ('completed', 'work-history', 'done', 'Already done')`);
    state.db
      .prepare(
        "INSERT INTO sessions (id, series_id, event_json) VALUES ('embedded', 'provider-series', ?)",
      )
      .run(JSON.stringify(identity));
    state.db.exec(
      readFileSync(
        join(
          __dirname,
          "../../../../../crates/db-app/migrations/20261117000000_scoped_series.sql",
        ),
        "utf8",
      ),
    );
    expect(
      queryAll(state.db, "SELECT id, series_id FROM sessions ORDER BY id"),
    ).toEqual([
      { id: "embedded", series_id: seriesId },
      {
        id: "home-history",
        series_id: seriesIdForEvent({ ...identity, calendar_id: "home" }),
      },
      { id: "manual", series_id: "manual-group" },
      { id: "work-history", series_id: seriesId },
    ]);
    expect(queryAll(state.db, "SELECT id, status FROM action_items")).toEqual([
      { id: "completed", status: "done" },
    ]);
  });
});
