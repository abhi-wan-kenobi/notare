import { renderHook } from "@testing-library/react";
import type { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAllActionItems } from "./queries";

import { createMigratedDb, queryAll } from "~/calendar/test-sqlite";

// Ahead of UTC, so the local calendar day rolls over before the UTC one.
process.env.TZ = "Asia/Kolkata";

let db: DatabaseSync;

vi.mock("~/shared/owner-user", () => ({ useOwnerUserId: () => "self" }));
vi.mock("~/db", () => ({
  useLiveQuery: ({
    sql,
    params,
    mapRows,
  }: {
    sql: string;
    params?: unknown[];
    mapRows?: (rows: unknown[]) => unknown;
  }) => {
    const rows = queryAll(db, sql, params);
    return { data: mapRows ? mapRows(rows) : rows, isLoading: false };
  },
}));

describe("action item ownership", () => {
  beforeEach(() => {
    db = createMigratedDb();
    db.exec(`
      INSERT INTO sessions(id, title, owner_user_id) VALUES ('meeting', 'Planning', 'self');
      INSERT INTO humans(id, name, owner_user_id) VALUES
        ('self', 'Me', 'self'), ('bob', 'Bob', 'self');
      INSERT INTO action_items(id, session_id, assignee_human_id, text, status) VALUES
        ('mine', 'meeting', 'self', 'My action', 'todo'),
        ('bobs', 'meeting', 'bob', 'Bob action', 'todo'),
        ('unassigned', 'meeting', '', 'Unassigned action', 'todo'),
        ('finished', 'meeting', 'self', 'Finished action', 'done');
    `);
  });
  afterEach(() => {
    vi.useRealTimers();
    db.close();
  });

  it("Mine excludes other attendees even when their contacts belong to me", () => {
    const { result } = renderHook(() => useAllActionItems("mine"));
    expect(result.current.items.map((item) => item.id)).toEqual(["mine"]);
  });

  it("Open still includes others and unassigned items without completed ones", () => {
    const { result } = renderHook(() => useAllActionItems("open"));
    expect(result.current.items.map((item) => item.id).sort()).toEqual([
      "bobs",
      "mine",
      "unassigned",
    ]);
  });

  it("Overdue rolls over at local midnight, not UTC midnight", () => {
    // 01:30 on Nov 18 in Asia/Kolkata while UTC is still on Nov 17.
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-17T20:00:00.000Z"));
    db.exec(`
      INSERT INTO action_items(id, session_id, text, status, due_at) VALUES
        ('due-yesterday', 'meeting', 'Due yesterday', 'todo', '2026-11-17'),
        ('due-today', 'meeting', 'Due today', 'todo', '2026-11-18');
    `);

    const { result } = renderHook(() => useAllActionItems("overdue"));

    expect(result.current.items.map((item) => item.id)).toEqual([
      "due-yesterday",
    ]);
  });
});
