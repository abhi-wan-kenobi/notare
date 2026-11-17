import { renderHook } from "@testing-library/react";
import type { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAllActionItems } from "./queries";

import { createMigratedDb, queryAll } from "~/calendar/test-sqlite";

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
  afterEach(() => db.close());

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
});
